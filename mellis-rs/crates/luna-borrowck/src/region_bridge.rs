//! Borrowck Bridge & Shadow Validation (REGION-01D-C).
//!
//! Connects the formal Region Engine (REGION-01) to mature Borrowck dataflow in shadow mode.
//!
//! ### Core Architectural Guarantees:
//! 1. **Three-Tier Decoupling**:
//!    `Carrier identity != Reference inference region != Provenance-source validity region`
//!    Dynamic boundary checks query the validity region of the *provenance source*,
//!    not the carrier's temporary inference region.
//! 2. **Pure Semantic Realization**:
//!    `PointDomainViolation` remains a pure semantic failure; diagnostic policy is owned by the adapter.
//! 3. **Consumes Dataflow Facts**:
//!    Consumes existing carrier liveness states (`live_before`, `live_after`, `live_at_entry`)
//!    and provenance sets without rebuilding dataflow.
//! 4. **4-State Shadow Verdict**:
//!    `Valid`, `Invalid`, `NotApplicable`, `Incomplete`. Unsupported facts are explicit gaps, never silent success.
//! 5. **Zero Unexplained Divergence**:
//!    All observed discrepancies must be strictly categorized.
//! 6. **No Capability Exclusivity Leakage**:
//!    Evaluates lifetime validity only, never loan exclusivity or mutable conflict checks.
//!
//! # Normative Authority Invariant (REGION-01D-D)
//! "Region authority determines whether a reference lifetime is valid.
//!  Borrow authority determines whether an otherwise lifetime-valid
//!  operation is legal under provenance, loan, capability, and move rules.
//!  Neither subsystem may reconstruct or override the other's verdict."

use std::collections::{HashMap, HashSet};
use luna_common::{Diagnostic, DiagnosticCode, Span};
use luna_mvir::{Function, Instruction, Operand, Terminator, ValueId, ValueOrigin};
use luna_semantic::{CaptureMode, CanonicalLifetimeContract, SemanticContext, SemanticType};
use luna_semantic::region::{
    CfgEdgeId, ConstraintKind, ConstraintOrigin, ProgramPointId, RealizationError, RealizationFacts,
    RegionGraph, RegionId, RegionKind, RegionRealization, RegionSolution,
};

use crate::cfg::LoopInfo;

/// Four-state validation verdict produced by the Region Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowRegionVerdict {
    /// Region engine proves the reference/carrier operation is valid.
    Valid,
    /// Region engine proves an invalid use or boundary violation.
    Invalid(RegionFailure),
    /// Operation does not involve references or validity regions.
    NotApplicable,
    /// Bridge cannot fully map the construct (explicit gap, never silent success).
    Incomplete(ShadowGap),
}

/// Category of semantic region failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionFailure {
    PointDomainViolation {
        region: RegionId,
        point: ProgramPointId,
        source: Operand,
        span: Option<Span>,
    },
    BoundaryViolation {
        carrier: ValueId,
        edge: CfgEdgeId,
        dependency_region: RegionId,
        terminated_region: RegionId,
        source: Operand,
        span: Option<Span>,
    },
    ReturnEscape {
        carrier: ValueId,
        reason: ReturnEscapeReason,
        span: Option<Span>,
    },
    UnsatisfiedContract {
        carrier: ValueId,
        reason: ContractViolationReason,
        span: Option<Span>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnEscapeReason {
    ClosureCapturesLocalBorrow,
    LocalVariableEscapes { source: Operand },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolationReason {
    ParameterNotContracted { param_index: u16 },
    DirectReturnWithoutProvenance,
}

impl RegionFailure {
    /// Maps structured semantic RegionFailure into user-facing Diagnostic.
    /// Preserves exact codes, messages, and spans.
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            RegionFailure::PointDomainViolation { span, .. } => {
                let mut diag = Diagnostic::error(
                    "error[E3005]: LocalBorrowEscape: Reference used outside its validity domain",
                )
                .with_code(DiagnosticCode::BorrowConflict);
                diag.span = span;
                diag
            }
            RegionFailure::BoundaryViolation { span, .. } => {
                let mut diag = Diagnostic::error(
                    "error[E3005]: LocalBorrowEscape: Reference to iteration-local variable escapes loop iteration",
                )
                .with_code(DiagnosticCode::BorrowConflict);
                diag.span = span;
                diag
            }
            RegionFailure::ReturnEscape { reason, span, .. } => {
                let mut diag = match reason {
                    ReturnEscapeReason::ClosureCapturesLocalBorrow => {
                        Diagnostic::error("error[E3005]: LocalBorrowEscape: Cannot return a closure that captures a local borrow")
                    }
                    ReturnEscapeReason::LocalVariableEscapes { .. } => {
                        Diagnostic::error("error[E3005]: LocalBorrowEscape: Reference to local variable escapes function scope")
                    }
                };
                diag.span = span;
                diag
            }
            RegionFailure::UnsatisfiedContract { reason, span, .. } => {
                let mut diag = match reason {
                    ContractViolationReason::ParameterNotContracted { param_index } => {
                        Diagnostic::error(format!(
                            "error[E2016]: LifetimeConstraintViolation: return value has provenance from parameter (index {}), which does not satisfy declared lifetime contract",
                            param_index
                        ))
                    }
                    ContractViolationReason::DirectReturnWithoutProvenance => {
                        Diagnostic::error(
                            "error[E2016]: LifetimeConstraintViolation: return expression does not satisfy declared lifetime contract",
                        )
                    }
                };
                diag.span = span;
                diag
            }
        }
    }
}

/// Explicitly tracked bridge representation gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowGap {
    UnsupportedOperand(Operand),
    UnmappedVariable(ValueId),
    UnmappedPoint(ValueId),
    UnmappedEdge((String, String)),
    ComplexAggregateProjection,
}

/// Outcome of the legacy borrowck analysis at a given observation point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyVerdict {
    Allowed,
    Rejected {
        diagnostic_code: String,
        message: String,
    },
}

/// Classification of alignment between legacy borrowck and the shadow Region engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DivergenceClassification {
    /// Legacy borrowck and Region engine agree completely.
    Agreement,
    /// Legacy borrowck bug where Region engine implements correct frozen semantics.
    LegacyBorrowckBug { reason: String },
    /// Discrepancy in adapter mapping or identifier translation.
    AdapterMappingBug { reason: String },
    /// Bug or gap in the Region engine implementation.
    RegionEngineBug { reason: String },
    /// A known unsupported bridge fact (explicit gap, never silent success).
    KnownUnsupportedGap { gap: ShadowGap },
    /// An unexplained divergence (MUST BE ZERO to close 01D-C!).
    Unexplained { details: String },
}

/// Recorded comparison between legacy borrowck and shadow Region engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowComparison {
    pub location: String,
    pub shadow_verdict: ShadowRegionVerdict,
    pub legacy_verdict: LegacyVerdict,
    pub classification: DivergenceClassification,
}

/// Intermediate container holding the synthesized `RegionGraph`, `RealizationFacts`,
/// and identity mappings between MVIR and Region IR.
pub struct RegionBorrowContext {
    pub graph: RegionGraph,
    pub facts: RealizationFacts,
    pub val_to_point: HashMap<ValueId, ProgramPointId>,
    pub edge_to_id: HashMap<(String, String), CfgEdgeId>,
    pub val_to_region: HashMap<ValueId, RegionId>,
    pub loop_to_region: HashMap<String, RegionId>,
    pub func_scope_region: RegionId,
    pub param_regions: HashMap<u16, RegionId>,
    pub value_origins: HashMap<ValueId, ValueOrigin>,
    pub value_spans: HashMap<ValueId, Option<Span>>,
    pub is_param_ref: HashMap<u16, bool>,
}

impl RegionBorrowContext {
    /// Synthesizes region graphs, constraints, points, and boundary facts from MVIR.
    pub fn build(func: &Function, loop_info: &LoopInfo, ctx: Option<&SemanticContext>) -> Self {
        let mut graph = RegionGraph::new();
        let mut facts = RealizationFacts::new();
        let mut val_to_point = HashMap::new();
        let mut edge_to_id = HashMap::new();
        let mut val_to_region = HashMap::new();
        let mut loop_to_region = HashMap::new();
        let mut param_regions = HashMap::new();
        let mut value_origins = HashMap::new();
        let mut value_spans = HashMap::new();
        let mut is_param_ref = HashMap::new();

        // 1. Root function body scope
        let func_scope_region = graph.add_region(RegionKind::Lexical { scope: 0 }).unwrap();

        // 1b. Parameter value mapping and structural outlives: R_param ⪰ R_func_scope
        for (idx, val_data) in func.values.iter().enumerate() {
            let vid = ValueId(idx as u32);
            value_origins.insert(vid, val_data.origin.clone());
            value_spans.insert(vid, val_data.span.clone());

            match &val_data.origin {
                ValueOrigin::Parameter(p_idx) => {
                    let r_param = graph.add_region(RegionKind::Contract).unwrap();
                    param_regions.insert(*p_idx as u16, r_param);
                    val_to_region.insert(vid, r_param);

                    // Structural outlives: parameter outlives function scope
                    let _ = graph.add_constraint(
                        ConstraintKind::Outlives {
                            sup: r_param,
                            sub: func_scope_region,
                        },
                        ConstraintOrigin::structural(),
                        val_data.span.clone(),
                    );

                    let is_ref = if let Some(c) = ctx {
                        matches!(c.types.get(val_data.ty), SemanticType::Reference(..))
                    } else {
                        false
                    };
                    is_param_ref.insert(*p_idx as u16, is_ref);
                }
                ValueOrigin::Global => {
                    val_to_region.insert(vid, graph.program_region());
                }
                _ => {}
            }
        }

        // 2. Loop iteration regions
        for (loop_idx, nl) in loop_info.natural_loops.iter().enumerate() {
            let loop_region = graph
                .add_region(RegionKind::Iteration {
                    loop_id: loop_idx as u32,
                })
                .unwrap();
            loop_to_region.insert(nl.header.clone(), loop_region);

            // Structural containment: func scope outlives loop iteration
            graph
                .add_constraint(
                    ConstraintKind::Outlives {
                        sup: func_scope_region,
                        sub: loop_region,
                    },
                    ConstraintOrigin::structural(),
                    None,
                )
                .unwrap();
        }

        // 3. Instruction points and value validity regions
        let mut all_points = HashSet::new();
        let mut loop_points: HashMap<String, HashSet<ProgramPointId>> = HashMap::new();

        for block in &func.blocks {
            for nl in &loop_info.natural_loops {
                if nl.body_blocks.contains(&block.label.name) {
                    for &val_id in &block.insts {
                        let point_id = ProgramPointId::from_raw(val_id.0);
                        loop_points
                            .entry(nl.header.clone())
                            .or_default()
                            .insert(point_id);
                    }
                }
            }

            for &val_id in &block.insts {
                let point_id = ProgramPointId::from_raw(val_id.0);
                val_to_point.insert(val_id, point_id);
                all_points.insert(point_id);

                // If value is not yet mapped (not parameter or global), assign its validity region.
                // In nested loops, the lifetime is bounded by the innermost enclosing loop.
                // NOTE(REGION-HARDENING): `min_by_key(|nl| nl.blocks.len())` safely identifies the
                // innermost natural loop under current loop discovery invariants (blocks(inner) ⊂ blocks(outer)).
                // Targeted for future hardening: replace block-count heuristic with explicit canonical LoopTree
                // nesting depth (`max_by_key(|nl| nl.depth)`).
                if !val_to_region.contains_key(&val_id) {
                    let enclosing_loop = loop_info
                        .natural_loops
                        .iter()
                        .filter(|nl| nl.local_values.contains(&val_id))
                        .min_by_key(|nl| nl.blocks.len());

                    if let Some(nl) = enclosing_loop {
                        if let Some(&loop_reg) = loop_to_region.get(&nl.header) {
                            val_to_region.insert(val_id, loop_reg);
                        } else {
                            val_to_region.insert(val_id, func_scope_region);
                        }
                    } else {
                        val_to_region.insert(val_id, func_scope_region);
                    }
                }
            }
        }

        // 4. Set allowed domains for lexical and iteration regions
        facts.set_allowed_domain(func_scope_region, all_points);
        for (header, points) in loop_points {
            if let Some(&loop_reg) = loop_to_region.get(&header) {
                facts.set_allowed_domain(loop_reg, points);
            }
        }

        // 5. CFG edges and boundary exits
        let mut edge_counter: u32 = 0;
        for block in &func.blocks {
            let successors = match &block.terminator {
                Some(Terminator::Br { target }) => vec![target.name.clone()],
                Some(Terminator::CondBr {
                    true_target,
                    false_target,
                    ..
                }) => vec![true_target.name.clone(), false_target.name.clone()],
                _ => vec![],
            };

            for succ in successors {
                let edge_key = (block.label.name.clone(), succ.clone());
                let edge_id = CfgEdgeId::from_raw(edge_counter);
                edge_counter += 1;
                edge_to_id.insert(edge_key, edge_id);

                // Check which loop iterations end across this edge
                for nl in &loop_info.natural_loops {
                    let is_back_edge = nl.header == succ && nl.blocks.contains(&block.label.name);
                    let is_exit_edge =
                        nl.blocks.contains(&block.label.name) && !nl.blocks.contains(&succ);
                    if is_back_edge || is_exit_edge {
                        if let Some(&loop_reg) = loop_to_region.get(&nl.header) {
                            facts.add_boundary_exit(edge_id, loop_reg);
                        }
                    }
                }
            }
        }

        // 6. Connect Reference borrowing constraints: R_referent ⪰ R_ref
        for block in &func.blocks {
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                if let Instruction::Borrow { base, .. } = &val_data.inst {
                    let r_ref = graph
                        .add_region(RegionKind::Inference { variable: val_id.0 })
                        .unwrap();

                    val_to_region.insert(val_id, r_ref);

                    let r_referent = match base {
                        Operand::Value(pv) => {
                            val_to_region.get(pv).copied().unwrap_or(func_scope_region)
                        }
                        Operand::Global(_) => graph.program_region(),
                        _ => func_scope_region,
                    };

                    let _ = graph.add_constraint(
                        ConstraintKind::Outlives {
                            sup: r_referent,
                            sub: r_ref,
                        },
                        ConstraintOrigin::borrow(),
                        val_data.span.clone(),
                    );
                }
            }
        }

        Self {
            graph,
            facts,
            val_to_point,
            edge_to_id,
            val_to_region,
            loop_to_region,
            func_scope_region,
            param_regions,
            value_origins,
            value_spans,
            is_param_ref,
        }
    }
}

/// The non-invasive Borrowck Bridge querying formal Region Realization in shadow mode.
pub struct RegionBorrowBridge<'a> {
    pub context: &'a RegionBorrowContext,
    pub solution: &'a RegionSolution<'a>,
    pub realization: &'a RegionRealization<'a>,
    aliases: HashMap<ValueId, Operand>,
}

impl<'a> RegionBorrowBridge<'a> {
    /// Constructs a `RegionBorrowBridge` wrapping the context and realization results.
    pub fn new(
        context: &'a RegionBorrowContext,
        solution: &'a RegionSolution<'a>,
        realization: &'a RegionRealization<'a>,
    ) -> Self {
        Self {
            context,
            solution,
            realization,
            aliases: HashMap::new(),
        }
    }

    /// Sets the current alias map from `BorrowStateData` for place resolution.
    pub fn with_aliases(mut self, aliases: HashMap<ValueId, Operand>) -> Self {
        self.aliases = aliases;
        self
    }

    /// Resolves an operand through the current alias map to its root place.
    pub fn resolve_operand<'b>(&'b self, op: &'b Operand) -> &'b Operand {
        let mut current = op;
        while let Operand::Value(v) = current {
            if let Some(alias) = self.aliases.get(v) {
                current = alias;
            } else {
                break;
            }
        }
        current
    }

    /// Evaluates whether a live carrier holding provenance `provenance` is legally valid
    /// at program point `at_point`.
    ///
    /// # Three-Tier Decoupling Invariant
    /// The bridge verifies validity for the **provenance source**, not the carrier inference region.
    pub fn check_carrier_use(
        &self,
        carrier: ValueId,
        provenance: &[Operand],
        at_point: ValueId,
    ) -> ShadowRegionVerdict {
        if provenance.is_empty() {
            return ShadowRegionVerdict::NotApplicable;
        }

        let Some(&point_id) = self.context.val_to_point.get(&at_point) else {
            return ShadowRegionVerdict::Incomplete(ShadowGap::UnmappedPoint(at_point));
        };

        // For multi-provenance joins (e.g. branch join P(r) = {x, y}),
        // conservative union requires every possible provenance source to be valid at this point.
        for op in provenance {
            let resolved = self.resolve_operand(op);
            let dep_region = match resolved {
                Operand::Global(_) => self.context.graph.program_region(),
                Operand::Value(pv) => {
                    if let Some(&reg) = self.context.val_to_region.get(pv) {
                        reg
                    } else {
                        return ShadowRegionVerdict::Incomplete(ShadowGap::UnmappedVariable(*pv));
                    }
                }
                other => return ShadowRegionVerdict::Incomplete(ShadowGap::UnsupportedOperand(other.clone())),
            };

            match self.realization.region_valid_at(dep_region, point_id) {
                Ok(true) => {}
                Ok(false) => {
                    return ShadowRegionVerdict::Invalid(RegionFailure::PointDomainViolation {
                        region: dep_region,
                        point: point_id,
                        source: op.clone(),
                        span: None,
                    });
                }
                Err(RealizationError::UninstantiatedAbstractRegion { .. }) => {
                    return ShadowRegionVerdict::Incomplete(ShadowGap::UnsupportedOperand(
                        Operand::Value(carrier),
                    ));
                }
                Err(_) => {
                    return ShadowRegionVerdict::Invalid(RegionFailure::PointDomainViolation {
                        region: dep_region,
                        point: point_id,
                        source: op.clone(),
                        span: None,
                    });
                }
            }
        }

        ShadowRegionVerdict::Valid
    }

    /// Evaluates whether a carrier holding provenance `provenance` is legally permitted
    /// to cross CFG edge `(from_block, to_block)`.
    ///
    /// # Dynamic Boundary Rule
    /// Crossing edge $E$ is invalid iff the reference's required validity depends on
    /// a dynamic region instance terminated by $E$.
    ///
    /// Crucially: `region_may_cross(region(S), edge)` is queried on the **provenance source**,
    /// not the carrier inference region. References created inside loops borrowing outer
    /// data have outer provenance ($R_{outer}$), which is legally permitted to cross iteration exits!
    pub fn check_boundary_crossing(
        &self,
        carrier: ValueId,
        provenance: &[Operand],
        from_block: &str,
        to_block: &str,
    ) -> ShadowRegionVerdict {
        if provenance.is_empty() {
            return ShadowRegionVerdict::NotApplicable;
        }

        let edge_key = (from_block.to_string(), to_block.to_string());
        let Some(&edge_id) = self.context.edge_to_id.get(&edge_key) else {
            return ShadowRegionVerdict::Incomplete(ShadowGap::UnmappedEdge(edge_key));
        };

        // If edge does not terminate any dynamic regions, crossing is unconditionally valid
        let Some(terminated) = self.context.facts.terminated_regions_at_edge(edge_id) else {
            return ShadowRegionVerdict::Valid;
        };
        if terminated.is_empty() {
            return ShadowRegionVerdict::Valid;
        }

        for op in provenance {
            let resolved = self.resolve_operand(op);
            let dep_region = match resolved {
                Operand::Global(_) => self.context.graph.program_region(),
                Operand::Value(pv) => {
                    if let Some(&reg) = self.context.val_to_region.get(pv) {
                        reg
                    } else {
                        return ShadowRegionVerdict::Incomplete(ShadowGap::UnmappedVariable(*pv));
                    }
                }
                other => return ShadowRegionVerdict::Incomplete(ShadowGap::UnsupportedOperand(other.clone())),
            };

            // Query: does dep_region depend on any dynamic instance terminated by edge_id?
            if !self.realization.region_may_cross(dep_region, edge_id) {
                let term = terminated.first().copied().unwrap_or(dep_region);
                return ShadowRegionVerdict::Invalid(RegionFailure::BoundaryViolation {
                    carrier,
                    edge: edge_id,
                    dependency_region: dep_region,
                    terminated_region: term,
                    source: op.clone(),
                    span: None,
                });
            }
        }

        ShadowRegionVerdict::Valid
    }

    /// Evaluates whether returning `return_val` with provenance `provenance` is legally valid
    /// from the perspective of the Region Engine.
    pub fn check_return(
        &self,
        return_val: ValueId,
        provenance: &[Operand],
        closure_modes: Option<&[CaptureMode]>,
        span: Option<Span>,
        declared_contract: Option<&CanonicalLifetimeContract>,
        is_ref_ret: bool,
        is_direct_ref_ret: bool,
    ) -> ShadowRegionVerdict {
        // 1. Check closure capture escape
        if let Some(modes) = closure_modes {
            if modes.iter().any(|m| matches!(m, CaptureMode::SharedBorrow | CaptureMode::MutableBorrow)) {
                return ShadowRegionVerdict::Invalid(RegionFailure::ReturnEscape {
                    carrier: return_val,
                    reason: ReturnEscapeReason::ClosureCapturesLocalBorrow,
                    span,
                });
            }
        }

        if !is_ref_ret && declared_contract.is_none() {
            return ShadowRegionVerdict::NotApplicable;
        }

        let mut has_local_escape = false;
        let mut actual_param_origins = HashSet::new();
        let mut has_global_origin = false;
        let mut escaping_source = None;

        for op in provenance {
            let resolved = self.resolve_operand(op);
            match resolved {
                Operand::Global(_) => {
                    has_global_origin = true;
                }
                Operand::Value(pv) => {
                    match self.context.value_origins.get(pv) {
                        Some(ValueOrigin::Global) => {
                            has_global_origin = true;
                        }
                        Some(ValueOrigin::Parameter(p_idx)) => {
                            let is_ref = self.context.is_param_ref.get(&(*p_idx as u16)).copied().unwrap_or(false);
                            if is_ref {
                                actual_param_origins.insert(*p_idx as u16);
                            } else {
                                has_local_escape = true;
                                escaping_source = Some(op.clone());
                            }
                        }
                        Some(ValueOrigin::Local) | Some(ValueOrigin::Temporary) => {
                            has_local_escape = true;
                            escaping_source = Some(op.clone());
                        }
                        None => {
                            has_local_escape = true;
                            escaping_source = Some(op.clone());
                        }
                    }
                }
                _ => {
                    has_local_escape = true;
                    escaping_source = Some(op.clone());
                }
            }
        }

        // If direct ref ret without loans:
        let ret_op = Operand::Value(return_val);
        if provenance.is_empty() && is_direct_ref_ret {
            let resolved = self.resolve_operand(&ret_op);
            match resolved {
                Operand::Global(_) => {
                    has_global_origin = true;
                }
                Operand::Value(pv) => {
                    match self.context.value_origins.get(pv) {
                        Some(ValueOrigin::Global) => {
                            has_global_origin = true;
                        }
                        Some(ValueOrigin::Parameter(p_idx)) => {
                            let is_ref = self.context.is_param_ref.get(&(*p_idx as u16)).copied().unwrap_or(false);
                            if is_ref {
                                actual_param_origins.insert(*p_idx as u16);
                            } else {
                                has_local_escape = true;
                                escaping_source = Some(resolved.clone());
                            }
                        }
                        Some(ValueOrigin::Local) | Some(ValueOrigin::Temporary) => {
                            has_local_escape = true;
                            escaping_source = Some(resolved.clone());
                        }
                        None => {
                            has_local_escape = true;
                            escaping_source = Some(resolved.clone());
                        }
                    }
                }
                _ => {
                    has_local_escape = true;
                    escaping_source = Some(resolved.clone());
                }
            }
        }

        if has_local_escape {
            return ShadowRegionVerdict::Invalid(RegionFailure::ReturnEscape {
                carrier: return_val,
                reason: ReturnEscapeReason::LocalVariableEscapes {
                    source: escaping_source.unwrap_or_else(|| Operand::Value(return_val)),
                },
                span,
            });
        }

        // Check declared contract satisfaction
        if let Some(contract) = declared_contract {
            if let Some(prov) = &contract.return_provenance {
                let allowed_indices: HashSet<u16> = prov.indices().iter().copied().collect();
                let invalid_origins: Vec<u16> = actual_param_origins
                    .iter()
                    .copied()
                    .filter(|idx| !allowed_indices.contains(idx))
                    .collect();

                if !invalid_origins.is_empty() {
                    return ShadowRegionVerdict::Invalid(RegionFailure::UnsatisfiedContract {
                        carrier: return_val,
                        reason: ContractViolationReason::ParameterNotContracted {
                            param_index: invalid_origins[0],
                        },
                        span,
                    });
                } else if is_direct_ref_ret && actual_param_origins.is_empty() && !has_global_origin {
                    return ShadowRegionVerdict::Invalid(RegionFailure::UnsatisfiedContract {
                        carrier: return_val,
                        reason: ContractViolationReason::DirectReturnWithoutProvenance,
                        span,
                    });
                }
            }
        }

        ShadowRegionVerdict::Valid
    }

    /// Authoritative diagnostic adapter for dynamic boundary crossing.
    pub fn diagnose_boundary_crossing(
        &self,
        carrier: ValueId,
        provenance: &[Operand],
        from_block: &str,
        to_block: &str,
        carrier_span: Option<Span>,
    ) -> Option<Diagnostic> {
        match self.check_boundary_crossing(carrier, provenance, from_block, to_block) {
            ShadowRegionVerdict::Invalid(mut failure) => {
                if let RegionFailure::BoundaryViolation { ref mut span, .. } = failure {
                    if span.is_none() {
                        *span = carrier_span;
                    }
                }
                Some(failure.into_diagnostic())
            }
            _ => None,
        }
    }

    /// Authoritative diagnostic adapter for return safety and contract satisfaction.
    pub fn diagnose_return(
        &self,
        return_val: ValueId,
        provenance: &[Operand],
        closure_modes: Option<&[CaptureMode]>,
        span: Option<Span>,
        declared_contract: Option<&CanonicalLifetimeContract>,
        is_ref_ret: bool,
        is_direct_ref_ret: bool,
    ) -> Option<Diagnostic> {
        match self.check_return(
            return_val,
            provenance,
            closure_modes,
            span,
            declared_contract,
            is_ref_ret,
            is_direct_ref_ret,
        ) {
            ShadowRegionVerdict::Invalid(failure) => Some(failure.into_diagnostic()),
            _ => None,
        }
    }

    /// Authoritative diagnostic adapter for carrier usage points.
    pub fn diagnose_carrier_use(
        &self,
        carrier: ValueId,
        provenance: &[Operand],
        at_point: ValueId,
        carrier_span: Option<Span>,
    ) -> Option<Diagnostic> {
        match self.check_carrier_use(carrier, provenance, at_point) {
            ShadowRegionVerdict::Invalid(mut failure) => {
                if let RegionFailure::PointDomainViolation { ref mut span, .. } = failure {
                    if span.is_none() {
                        *span = carrier_span;
                    }
                }
                Some(failure.into_diagnostic())
            }
            _ => None,
        }
    }

    /// Evaluates and classifies the comparison between the shadow Region verdict and legacy verdict.
    pub fn evaluate_comparison(
        &self,
        location: String,
        shadow: ShadowRegionVerdict,
        legacy: LegacyVerdict,
    ) -> ShadowComparison {
        let classification = match (&shadow, &legacy) {
            (ShadowRegionVerdict::Valid, LegacyVerdict::Allowed) => {
                DivergenceClassification::Agreement
            }
            (ShadowRegionVerdict::Invalid(_), LegacyVerdict::Rejected { .. }) => {
                DivergenceClassification::Agreement
            }
            (ShadowRegionVerdict::NotApplicable, LegacyVerdict::Allowed) => {
                DivergenceClassification::Agreement
            }
            (ShadowRegionVerdict::Incomplete(gap), _) => {
                DivergenceClassification::KnownUnsupportedGap { gap: gap.clone() }
            }
            (ShadowRegionVerdict::Valid, LegacyVerdict::Rejected { diagnostic_code, message }) => {
                DivergenceClassification::Unexplained {
                    details: format!(
                        "Shadow engine says Valid, but legacy borrowck rejected: [{}] {}",
                        diagnostic_code, message
                    ),
                }
            }
            (ShadowRegionVerdict::Invalid(failure), LegacyVerdict::Allowed) => {
                DivergenceClassification::Unexplained {
                    details: format!(
                        "Shadow engine says Invalid ({:?}), but legacy borrowck allowed",
                        failure
                    ),
                }
            }
            (ShadowRegionVerdict::NotApplicable, LegacyVerdict::Rejected { diagnostic_code, message }) => {
                DivergenceClassification::Unexplained {
                    details: format!(
                        "Shadow engine says NotApplicable, but legacy borrowck rejected: [{}] {}",
                        diagnostic_code, message
                    ),
                }
            }
        };

        ShadowComparison {
            location,
            shadow_verdict: shadow,
            legacy_verdict: legacy,
            classification,
        }
    }
}
