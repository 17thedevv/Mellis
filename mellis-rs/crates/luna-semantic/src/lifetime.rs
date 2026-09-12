//! Lifetime system for Mellis.
//!
//! This module implements the lifetime expression system according to the
//! Implementation Contract specification.
//!
//! ## Lifetime Domain
//! ```text
//! 'a ≤ 'b  ⟺  region('a) ⊆ region('b)
//! 'static = top
//! ⊥ = empty (abstract)
//!
//! meet('a, 'b) = greatest region ⊆ 'a and ⊆ 'b
//! ```
//!
//! ## Provenance Domain
//! ```text
//! P ::= Input(x)           // from parameter
//!     | Place(x)            // address of local
//!     | Field(p, f)       // field access
//!     | Deref(p)          // dereference
//!     | Reborrow(p)        // reference from reference
//!     | ProvenanceSet(Vec) // canonicalized alternatives
//!     | SymbolicLoop(id,x) // internal only
//! ```
//!
//! ## User Syntax
//! ```text
//! life_from(a)              // origin from a
//! life_from(a | b | c)      // origin from any of them
//! where outlives(a, b)      // constraint: 'b ≤ 'a
//! ```

use crate::{SemanticContext, SymbolId};
use crate::ty::LifetimeId;
use luna_common::Span;
use crate::symbol::SymbolTable;
use luna_ast::{FnLifetimeSignature, LifetimeExpr, LifetimeConstraint};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, BTreeMap, BTreeSet};

/// A lifetime identifier used during semantic analysis.
/// This is distinct from the raw identifier in the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LifetimeIdent(pub u32);

impl From<u32> for LifetimeIdent {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

/// A lifetime variable that can be unified during constraint solving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LifetimeVar(pub u32);

/// A constraint on lifetimes: `'b ≤ 'a` (b outlives a).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LifetimeConstraintExpr {
    /// The lifetime that must outlive the other (longer or equal).
    pub longer: LifetimeIdent,
    /// The lifetime that must be outlived (shorter or equal).
    pub shorter: LifetimeIdent,
}

impl LifetimeConstraintExpr {
    /// Creates a new outlives constraint: `longer ≤ shorter` (longer outlives shorter).
    /// In the lattice: `'longer ≤ 'shorter` means region(longer) ⊆ region(shorter).
    pub fn outlives(longer: LifetimeIdent, shorter: LifetimeIdent) -> Self {
        Self { longer, shorter }
    }
}

/// Provenance tracking for references.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Provenance from a parameter.
    Input(SymbolId),
    /// Provenance from a local variable.
    Place(SymbolId),
    /// Provenance from a field access.
    Field(Box<Provenance>, SymbolId),
    /// Provenance from a dereference.
    Deref(Box<Provenance>),
    /// Provenance from a reborrow.
    Reborrow(Box<Provenance>),
    /// A set of alternative provenances (canonicalized).
    ProvenanceSet(Vec<Provenance>),
    /// Internal provenance for loop analysis (never user-visible).
    SymbolicLoop(u32, SymbolId),
}

impl Provenance {
    /// Canonicalizes the provenance set.
    /// - `{a}` ≡ `a`
    /// - `{a, b}` ≡ `{b, a}` (sorted)
    /// - `{a, {b, c}}` ≡ `{a, b, c}` (flattened)
    pub fn canonicalize(self) -> Self {
        match self {
            Provenance::ProvenanceSet(elements) => {
                if elements.len() == 1 {
                    return elements.into_iter().next().unwrap().canonicalize();
                }

                let mut flattened: Vec<Provenance> = Vec::new();
                for elem in elements {
                    match elem.canonicalize() {
                        Provenance::ProvenanceSet(inner) => {
                            flattened.extend(inner);
                        }
                        other => flattened.push(other),
                    }
                }

                // Sort for canonical representation
                flattened.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
                flattened.dedup();

                Provenance::ProvenanceSet(flattened)
            }
            other => other,
        }
    }
}

/// A resolved lifetime expression with provenance information.
#[derive(Debug, Clone)]
pub struct ResolvedLifetimeExpr {
    pub provenance: Provenance,
    pub lifetime_id: LifetimeIdent,
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provenance::Input(sym) => write!(f, "Input({})", sym.0),
            Provenance::Place(sym) => write!(f, "Place({})", sym.0),
            Provenance::Field(p, sym_id) => write!(f, "Field({}, {})", p, sym_id.0),
            Provenance::Deref(p) => write!(f, "Deref({})", p),
            Provenance::Reborrow(p) => write!(f, "Reborrow({})", p),
            Provenance::ProvenanceSet(ps) => {
                write!(f, "{{")?;
                for (i, p) in ps.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, "}}")
            }
            Provenance::SymbolicLoop(id, sym) => write!(f, "Loop({}, {})", id, sym.0),
        }
    }
}

/// Result of lifetime solving.
#[derive(Debug, Clone)]
pub enum SolveResult {
    /// A satisfiable solution exists.
    Sat(LifetimeAssignment),
    /// No satisfiable solution exists.
    Unsat(UnsatisfiableConstraint),
    /// Analysis failed due to resource limits (not a type error).
    AnalysisFailure(ResourceLimit),
}

/// The solved lifetime assignment.
#[derive(Debug, Clone)]
pub struct LifetimeAssignment {
    /// Map from lifetime identifier to its resolved lifetime.
    pub lifetimes: BTreeMap<LifetimeIdent, LifetimeIdent>,
    /// Map from lifetime variable to its bound value.
    pub vars: BTreeMap<LifetimeVar, LifetimeIdent>,
}

/// An unsatisfiable constraint error.
#[derive(Debug, Clone)]
pub struct UnsatisfiableConstraint {
    pub constraint: LifetimeConstraintExpr,
    pub reason: String,
}

/// Resource limit exceeded.
#[derive(Debug, Clone)]
pub struct ResourceLimit {
    pub kind: String,
}

/// Lifetime solver that performs constraint solving.
pub struct LifetimeSolver {
    /// All lifetime constraints.
    constraints: Vec<LifetimeConstraintExpr>,
    /// Lifetime variables to be solved.
    vars: BTreeMap<LifetimeVar, LifetimeIdent>,
    /// Map from identifier to lifetime (for named lifetimes).
    idents: BTreeMap<LifetimeIdent, LifetimeIdent>,
    /// Next available lifetime identifier.
    next_lifetime_id: u32,
    /// Next available lifetime variable.
    next_var_id: u32,
}

impl LifetimeSolver {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            vars: BTreeMap::new(),
            idents: BTreeMap::new(),
            next_lifetime_id: 0,
            next_var_id: 0,
        }
    }

    /// Creates a fresh lifetime variable.
    pub fn fresh_var(&mut self) -> LifetimeVar {
        let var = LifetimeVar(self.next_var_id);
        self.next_var_id += 1;
        self.vars.insert(var, LifetimeIdent(0)); // placeholder
        var
    }

    /// Creates a fresh lifetime identifier.
    pub fn fresh_ident(&mut self) -> LifetimeIdent {
        let ident = LifetimeIdent(self.next_lifetime_id);
        self.next_lifetime_id += 1;
        ident
    }

    /// Registers a named lifetime identifier.
    pub fn register_lifetime(&mut self, name: &str, ident: LifetimeIdent) {
        // Named lifetimes are identity-mapped to themselves
        self.idents.insert(ident, ident);
    }

    /// Adds a lifetime constraint.
    pub fn add_constraint(&mut self, constraint: LifetimeConstraintExpr) {
        self.constraints.push(constraint);
    }

    /// Solves the lifetime constraints.
    pub fn solve(&mut self) -> SolveResult {
        // Build constraint graph
        let mut graph: BTreeMap<LifetimeIdent, BTreeSet<LifetimeIdent>> = BTreeMap::new();

        for constraint in &self.constraints {
            // constraint: longer ≤ shorter means region(longer) ⊆ region(shorter)
            // So shorter is reachable from longer (longer points to shorter)
            graph.entry(constraint.longer).or_default().insert(constraint.shorter);
        }

        // Find strongly connected components (SCCs) using Tarjan's algorithm
        let sccs = self.find_sccs(&graph);

        // Collapse SCCs - lifetimes in the same SCC are equal
        let mut repr: BTreeMap<LifetimeIdent, LifetimeIdent> = BTreeMap::new();
        for scc in &sccs {
            let canonical = scc.iter().min().copied().unwrap_or(LifetimeIdent(0));
            for &ident in scc {
                repr.insert(ident, canonical);
            }
        }

        // Check for ⊥ contradictions: 'a ≤ ⊥ means nothing can outlive empty
        for constraint in &self.constraints {
            // If we have 'a ≤ ⊥ where ⊥ is a special bottom value, that's unsatisfiable
            // For now, we just require that no constraint implies a cycle to ⊥
            if constraint.longer == constraint.shorter {
                // 'a ≤ 'a is fine (reflexive)
                continue;
            }
        }

        // Topological solve: check for cycles that would make constraints unsatisfiable
        for constraint in &self.constraints {
            let longer_canonical = *repr.get(&constraint.longer).unwrap_or(&constraint.longer);
            let shorter_canonical = *repr.get(&constraint.shorter).unwrap_or(&constraint.shorter);

            if longer_canonical == shorter_canonical {
                // This is fine - lifetimes in the same SCC are equal
                continue;
            }

            // Check if there's a path from shorter back to longer (cycle)
            if self.has_path(&graph, shorter_canonical, longer_canonical, &repr) {
                return SolveResult::Unsat(UnsatisfiableConstraint {
                    constraint: LifetimeConstraintExpr {
                        longer: constraint.longer,
                        shorter: constraint.shorter,
                    },
                    reason: "circular lifetime constraint".to_string(),
                });
            }
        }

        // Build final assignment
        let mut lifetimes = BTreeMap::new();
        for (ident, _) in &self.idents {
            lifetimes.insert(*ident, *repr.get(ident).unwrap_or(ident));
        }

        let vars: BTreeMap<LifetimeVar, LifetimeIdent> = self.vars.iter()
            .map(|(&var, _)| {
                // Each var is bound to its canonical representative
                let canonical = *repr.get(&LifetimeIdent(var.0)).unwrap_or(&LifetimeIdent(var.0));
                (var, canonical)
            })
            .collect();

        SolveResult::Sat(LifetimeAssignment { lifetimes, vars })
    }

    /// Find strongly connected components using Tarjan's algorithm.
    fn find_sccs(&self, graph: &BTreeMap<LifetimeIdent, BTreeSet<LifetimeIdent>>) -> Vec<BTreeSet<LifetimeIdent>> {
        let mut index = 0u32;
        let mut stack: Vec<LifetimeIdent> = Vec::new();
        let mut on_stack: BTreeSet<LifetimeIdent> = BTreeSet::new();
        let mut indices: BTreeMap<LifetimeIdent, u32> = BTreeMap::new();
        let mut lowlinks: BTreeMap<LifetimeIdent, u32> = BTreeMap::new();
        let mut sccs: Vec<BTreeSet<LifetimeIdent>> = Vec::new();

        for &node in graph.keys() {
            if !indices.contains_key(&node) {
                self.tarjan_scc(
                    node,
                    graph,
                    &mut index,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut lowlinks,
                    &mut sccs,
                );
            }
        }

        sccs
    }

    fn tarjan_scc(
        &self,
        node: LifetimeIdent,
        graph: &BTreeMap<LifetimeIdent, BTreeSet<LifetimeIdent>>,
        index: &mut u32,
        stack: &mut Vec<LifetimeIdent>,
        on_stack: &mut BTreeSet<LifetimeIdent>,
        indices: &mut BTreeMap<LifetimeIdent, u32>,
        lowlinks: &mut BTreeMap<LifetimeIdent, u32>,
        sccs: &mut Vec<BTreeSet<LifetimeIdent>>,
    ) {
        indices.insert(node, *index);
        lowlinks.insert(node, *index);
        *index += 1;
        stack.push(node);
        on_stack.insert(node);

        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                if !indices.contains_key(&neighbor) {
                    self.tarjan_scc(neighbor, graph, index, stack, on_stack, indices, lowlinks, sccs);
                    let neighbor_low = *lowlinks.get(&neighbor).unwrap_or(&0);
                    let node_low = *lowlinks.entry(node).or_insert(0);
                    let min = node_low.min(neighbor_low);
                    lowlinks.insert(node, min);
                } else if on_stack.contains(&neighbor) {
                    let neighbor_index = *indices.get(&neighbor).unwrap_or(&0);
                    let node_low = *lowlinks.entry(node).or_insert(0);
                    lowlinks.insert(node, node_low.min(neighbor_index));
                }
            }
        }

        if lowlinks.get(&node) == indices.get(&node) {
            let mut scc = BTreeSet::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                scc.insert(w);
                if w == node {
                    break;
                }
            }
            sccs.push(scc);
        }
    }

    /// Check if there's a path from source to target in the graph.
    fn has_path(
        &self,
        graph: &BTreeMap<LifetimeIdent, BTreeSet<LifetimeIdent>>,
        source: LifetimeIdent,
        target: LifetimeIdent,
        repr: &BTreeMap<LifetimeIdent, LifetimeIdent>,
    ) -> bool {
        let mut visited: BTreeSet<LifetimeIdent> = BTreeSet::new();
        let mut queue = vec![source];

        while let Some(current) = queue.pop() {
            if current == target {
                return true;
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if let Some(neighbors) = graph.get(&current) {
                for &neighbor in neighbors {
                    let canonical = *repr.get(&neighbor).unwrap_or(&neighbor);
                    if !visited.contains(&canonical) {
                        queue.push(canonical);
                    }
                }
            }
        }

        false
    }
}

impl Default for LifetimeSolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Error types for lifetime analysis.
#[derive(Debug, Clone)]
pub enum LifetimeError {
    /// Constraint cannot be satisfied.
    UnsatisfiableConstraint(UnsatisfiableConstraint),
    /// Type inconsistency in alternatives.
    TypeInconsistency {
        expected: String,
        found: String,
        span: Span,
    },
    /// Unresolved lifetime identifier.
    UnresolvedLifetime {
        name: String,
        span: Span,
    },
    /// Analysis timeout.
    AnalysisTimeout,
    /// Return provenance mismatch.
    ReturnProvenanceMismatch {
        expected: String,
        found: String,
        span: Span,
    },
    /// Function call constraint violation.
    ConstraintViolation {
        callee: String,
        constraint: String,
        span: Span,
    },
}

impl LifetimeError {
    pub fn into_diagnostic(self) -> luna_common::Diagnostic {
        match self {
            LifetimeError::UnsatisfiableConstraint(c) => {
                luna_common::Diagnostic::error(format!(
                    "lifetime constraint '{} ≤ {}' cannot be satisfied: {}",
                    c.constraint.shorter.0, c.constraint.longer.0, c.reason
                ))
            }
            LifetimeError::TypeInconsistency { expected, found, span } => {
                luna_common::Diagnostic::error(format!(
                    "type mismatch in lifetime alternatives: expected {}, found {}",
                    expected, found
                )).with_span(span)
            }
            LifetimeError::UnresolvedLifetime { name, span } => {
                luna_common::Diagnostic::error(format!(
                    "lifetime '{}' does not refer to any parameter in scope",
                    name
                )).with_span(span)
            }
            LifetimeError::AnalysisTimeout => {
                luna_common::Diagnostic::error("lifetime analysis exceeded resource limit")
            }
            LifetimeError::ReturnProvenanceMismatch { expected, found, span } => {
                luna_common::Diagnostic::error(format!(
                    "return value provenance mismatch: expected '{}', found '{}'",
                    expected, found
                )).with_span(span)
            }
            LifetimeError::ConstraintViolation { callee, constraint, span } => {
                luna_common::Diagnostic::error(format!(
                    "function '{}' requires {}, but condition not satisfied",
                    callee, constraint
                )).with_span(span)
            }
        }
    }
}



/// Validator for lifetime expressions.
pub struct LifetimeValidator<'a> {
    /// Maps parameter/local names to their lifetime identifiers.
    name_to_lifetime: HashMap<String, LifetimeIdent>,
    /// Type context for checking type consistency.
    types: &'a crate::ty::TypeContext,
    source_manager: &'a luna_common::source::SourceManager,
}

impl<'a> LifetimeValidator<'a> {
    pub fn new(types: &'a crate::ty::TypeContext, source_manager: &'a luna_common::source::SourceManager) -> Self {
        Self {
            name_to_lifetime: HashMap::new(),
            types,
            source_manager,
        }
    }

    /// Registers a parameter with its lifetime.
    pub fn register_param(&mut self, name: &str, lifetime: LifetimeIdent) {
        self.name_to_lifetime.insert(name.to_string(), lifetime);
    }

    /// Resolves a lifetime expression from the AST.
    pub fn resolve_lifetime_expr(
        &self,
        expr: &LifetimeExpr,
        solver: &mut LifetimeSolver,
    ) -> Result<ResolvedLifetimeExpr, LifetimeError> {
        match expr {
            LifetimeExpr::Provenance(span) => {
                let name = extract_name_from_span(span, self.source_manager);
                let lifetime = self.name_to_lifetime.get(&name)
                    .copied()
                    .ok_or_else(|| LifetimeError::UnresolvedLifetime { name: name.clone(), span: *span })?;

                let provenance = Provenance::Input(SymbolId(0)); // TODO: map to actual SymbolId
                let resolved = ResolvedLifetimeExpr {
                    provenance,
                    lifetime_id: lifetime,
                };
                Ok(resolved)
            }
            LifetimeExpr::ProvenanceSet(idents) => {
                let mut provenances = Vec::new();
                let mut lifetime_idents = Vec::new();

                for span in idents {
                    let name = extract_name_from_span(span, self.source_manager);
                    let lifetime = self.name_to_lifetime.get(&name)
                        .copied()
                        .ok_or_else(|| LifetimeError::UnresolvedLifetime { name: name.clone(), span: *span })?;
                    lifetime_idents.push(lifetime);
                    provenances.push(Provenance::Input(SymbolId(0)));
                }

                // Create constraint: return lifetime ≤ each input lifetime
                let return_var = solver.fresh_var();
                for &input_lifetime in &lifetime_idents {
                    solver.add_constraint(LifetimeConstraintExpr::outlives(input_lifetime, return_var.0.into()));
                }

                let provenance = Provenance::ProvenanceSet(provenances).canonicalize();
                Ok(ResolvedLifetimeExpr {
                    provenance,
                    lifetime_id: return_var.0.into(),
                })
            }
        }
    }

    /// Resolves a lifetime constraint from the AST.
    pub fn resolve_constraint(
        &self,
        constraint: &LifetimeConstraint,
        solver: &mut LifetimeSolver,
    ) -> Result<(), LifetimeError> {
        let first_name = extract_name_from_span(&constraint.first, self.source_manager);
        let second_name = extract_name_from_span(&constraint.second, self.source_manager);

        let first_lifetime = self.name_to_lifetime.get(&first_name)
            .copied()
            .ok_or_else(|| LifetimeError::UnresolvedLifetime { name: first_name.clone(), span: constraint.first })?;
        let second_lifetime = self.name_to_lifetime.get(&second_name)
            .copied()
            .ok_or_else(|| LifetimeError::UnresolvedLifetime { name: second_name.clone(), span: constraint.second })?;

        // outlives(a, b) → lifetime(a) ≥ lifetime(b) ('a outlives 'b)
        // That means: longer = first_lifetime ('a), shorter = second_lifetime ('b)
        solver.add_constraint(LifetimeConstraintExpr {
            longer: first_lifetime,
            shorter: second_lifetime,
        });

        Ok(())
    }
}

pub fn extract_name_from_span(span: &Span, source_manager: &luna_common::source::SourceManager) -> String {
    if let Some(text) = source_manager.get_file(span.file_id).unwrap().source.get(span.start as usize .. span.end as usize) {
        text.to_string()
    } else {
        format!("_unnamed_{}_{}", span.start, span.end)
    }
}

/// Resolves a function's lifetime signature.
pub fn resolve_fn_lifetime_signature(
    sig: &FnLifetimeSignature,
    param_names: &[String],
    ctx: &mut SemanticContext,
    source_manager: &luna_common::source::SourceManager,
) -> Result<(Option<ResolvedLifetimeExpr>, Vec<LifetimeConstraintExpr>), LifetimeError> {
    let mut solver = LifetimeSolver::new();
    let mut validator = LifetimeValidator::new(&ctx.types, source_manager);

    // Register parameters with their lifetimes
    for (i, name) in param_names.iter().enumerate() {
        let lifetime = LifetimeIdent(i as u32);
        solver.register_lifetime(name, lifetime);
        validator.register_param(name, lifetime);
    }

    // Resolve provenance expression
    let expr_result = if let Some(ref expr) = sig.provenance {
        Some(validator.resolve_lifetime_expr(expr, &mut solver)?)
    } else {
        None
    };

    // Resolve constraints
    let mut constraints = Vec::new();
    for constraint in &sig.constraints {
        validator.resolve_constraint(constraint, &mut solver)?;
        constraints.push(LifetimeConstraintExpr {
            longer: LifetimeIdent(0), // placeholder
            shorter: LifetimeIdent(0), // placeholder
        });
    }

    // Solve constraints
    match solver.solve() {
        SolveResult::Sat(assignment) => {
            // TODO: use assignment to update lifetime information
            Ok((expr_result, constraints))
        }
        SolveResult::Unsat(err) => Err(LifetimeError::UnsatisfiableConstraint(err)),
        SolveResult::AnalysisFailure(_) => Err(LifetimeError::AnalysisTimeout),
    }
}

pub fn check_fn_call_constraints(
    callee_decl: &luna_ast::Decl,
    caller_args: &[luna_ast::ExprId],
    ctx: &mut SemanticContext,
    source_manager: &luna_common::source::SourceManager,
    arena: &luna_ast::AstArena,
    _call_span: Span,
) -> Result<(), LifetimeError> {
    let luna_ast::Decl::Function { params, lifetime_signature, .. } = callee_decl else {
        return Ok(());
    };

    let find_param_index = |target_name: &str| -> Option<usize> {
        params.iter().position(|&param_id| {
            if let luna_ast::Decl::Param { name, .. } = &arena.decls[param_id.0 as usize] {
                extract_name_from_span(name, source_manager) == target_name
            } else {
                false
            }
        })
    };

    for constraint in &lifetime_signature.constraints {
        let first_name = extract_name_from_span(&constraint.first, source_manager);
        let second_name = extract_name_from_span(&constraint.second, source_manager);

        let Some(first_param_idx) = find_param_index(&first_name) else {
            return Err(LifetimeError::UnresolvedLifetime { name: first_name, span: constraint.first });
        };
        let Some(second_param_idx) = find_param_index(&second_name) else {
            return Err(LifetimeError::UnresolvedLifetime { name: second_name, span: constraint.second });
        };

        let first_arg = caller_args.get(first_param_idx).copied();
        let second_arg = caller_args.get(second_param_idx).copied();
        
        if let (Some(first_arg), Some(second_arg)) = (first_arg, second_arg) {
            let first_lifetime = ctx.tables.expr_lifetimes.get(&first_arg).copied();
            let second_lifetime = ctx.tables.expr_lifetimes.get(&second_arg).copied();

            if first_lifetime.is_none() || second_lifetime.is_none() {
                continue;
            }
        }
    }

    Ok(())
}

pub fn check_return_provenance(
    decl: &luna_ast::Decl,
    _return_expr: luna_ast::ExprId,
    _ctx: &mut SemanticContext,
    _source_manager: &luna_common::source::SourceManager,
    _arena: &luna_ast::AstArena,
    _return_span: Span,
) -> Result<(), LifetimeError> {
    let luna_ast::Decl::Function { lifetime_signature, .. } = decl else {
        return Ok(());
    };

    let Some(_provenance) = &lifetime_signature.provenance else {
        return Ok(());
    };
    
    Ok(())
}

pub struct LifetimeVerifier<'a> {
    ctx: &'a SemanticContext,
    source_manager: &'a luna_common::source::SourceManager,
    arena: &'a luna_ast::AstArena,
    params: &'a [luna_ast::DeclId],
}

impl<'a> LifetimeVerifier<'a> {
    pub fn new(ctx: &'a SemanticContext, source_manager: &'a luna_common::source::SourceManager, arena: &'a luna_ast::AstArena, params: &'a [luna_ast::DeclId]) -> Self {
        Self { ctx, source_manager, arena, params }
    }

    pub fn verify_fn_signature(&self, decl: &luna_ast::Decl) -> Result<(), LifetimeError> {
        let luna_ast::Decl::Function { lifetime_signature, .. } = decl else {
            return Ok(());
        };

        if let Some(provenance) = &lifetime_signature.provenance {
            self.verify_provenance(provenance)?;
        }

        for constraint in &lifetime_signature.constraints {
            self.verify_constraint(constraint)?;
        }

        Ok(())
    }

    fn verify_provenance(&self, provenance: &LifetimeExpr) -> Result<(), LifetimeError> {
        match provenance {
            LifetimeExpr::Provenance(span) => {
                let name = extract_name_from_span(span, self.source_manager);
                if !self.is_valid_parameter(&name) {
                    return Err(LifetimeError::UnresolvedLifetime {
                        name,
                        span: *span,
                    });
                }
            }
            LifetimeExpr::ProvenanceSet(idents) => {
                for span in idents {
                    let name = extract_name_from_span(span, self.source_manager);
                    if !self.is_valid_parameter(&name) {
                        return Err(LifetimeError::UnresolvedLifetime {
                            name,
                            span: *span,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn verify_constraint(&self, constraint: &LifetimeConstraint) -> Result<(), LifetimeError> {
        let first_name = extract_name_from_span(&constraint.first, self.source_manager);
        let second_name = extract_name_from_span(&constraint.second, self.source_manager);

        if !self.is_valid_parameter(&first_name) {
            return Err(LifetimeError::UnresolvedLifetime {
                name: first_name,
                span: constraint.first,
            });
        }
        if !self.is_valid_parameter(&second_name) {
            return Err(LifetimeError::UnresolvedLifetime {
                name: second_name,
                span: constraint.second,
            });
        }
        Ok(())
    }

    fn is_valid_parameter(&self, name: &str) -> bool {
        self.params.iter().any(|&param_id| {
            if let luna_ast::Decl::Param { name: p_name, .. } = &self.arena.decls[param_id.0 as usize] {
                extract_name_from_span(p_name, self.source_manager) == name
            } else {
                false
            }
        })
    }
}

pub fn verify_before_codegen(
    decl: &luna_ast::Decl,
    ctx: &SemanticContext,
    source_manager: &luna_common::source::SourceManager,
    arena: &luna_ast::AstArena,
) -> Result<(), LifetimeError> {
    if let luna_ast::Decl::Function { params, .. } = decl {
        let verifier = LifetimeVerifier::new(ctx, source_manager, arena, params);
        verifier.verify_fn_signature(decl)
    } else {
        Ok(())
    }
}

// =========================================================================
// Canonical Lifetime Relation ABI (SSOT)
// =========================================================================

/// Current version of the canonical lifetime relation ABI contract.
pub const LIFETIME_RELATION_ABI_VERSION: u32 = 1;

/// Canonical provenance of a returned reference.
/// Represents the parameter origin(s) of a return reference using 0-indexed parameter positions.
///
/// Invariants:
/// - Uses 0-indexed canonical parameter position, never parameter names or session-local IDs.
/// - `Param(i)`: Return reference derives provenance strictly from parameter `i` (`life_from(p_i)`).
/// - `ParamSet(indices)`: Return reference may derive provenance from any parameter in `indices` (`life_from(p_i | p_j)`).
///   `indices` is strictly sorted in ascending order, deduplicated, and has length >= 2.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CanonicalProvenance {
    /// Origin from a single parameter index.
    Param(u16),
    /// Origin from any of several parameter indices (alternative origins / union of bounds).
    ParamSet(Vec<u16>),
}

impl CanonicalProvenance {
    /// Canonicalizes a collection of parameter indices into a canonical provenance.
    ///
    /// Properties:
    /// - Deduplicates identical parameter indices.
    /// - Sorts indices in ascending order (deterministic).
    /// - Collapses single-element sets into `CanonicalProvenance::Param(idx)`.
    /// - Returns `None` if the index list is empty.
    pub fn from_indices(mut indices: Vec<u16>) -> Option<Self> {
        indices.sort_unstable();
        indices.dedup();
        match indices.len() {
            0 => None,
            1 => Some(CanonicalProvenance::Param(indices[0])),
            _ => Some(CanonicalProvenance::ParamSet(indices)),
        }
    }

    /// Returns the slice of parameter indices that may contribute to this provenance.
    pub fn indices(&self) -> &[u16] {
        match self {
            CanonicalProvenance::Param(idx) => std::slice::from_ref(idx),
            CanonicalProvenance::ParamSet(indices) => indices.as_slice(),
        }
    }
}

/// Canonical representation of an outlives constraint between two parameter lifetimes:
/// `where outlives(longer, shorter)`
///
/// Direction semantics:
/// - `longer` parameter outlives `shorter` parameter: `lifetime(longer) >= lifetime(shorter)`
///   (the memory region of `longer` is valid at least as long as `shorter`).
///
/// Invariants:
/// - `longer != shorter` (reflexive constraints are redundant and filtered during normalization).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CanonicalOutlivesConstraint {
    /// 0-indexed position of the parameter that must outlive `shorter`.
    pub longer: u16,
    /// 0-indexed position of the parameter that is outlived by `longer`.
    pub shorter: u16,
}

impl CanonicalOutlivesConstraint {
    pub fn new(longer: u16, shorter: u16) -> Self {
        Self { longer, shorter }
    }
}

/// Canonical lifetime relation contract for a callable function signature.
/// This is the Single Source of Truth (SSOT) for lifetime relationships across boundaries.
///
/// It encapsulates:
/// 1. ABI contract version (`version`)
/// 2. Return reference provenance (`return_provenance`), if any
/// 3. Outlives relations between parameters (`outlives_constraints`)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct CanonicalLifetimeContract {
    /// Version of the lifetime relation model.
    pub version: u32,
    /// Provenance of the returned reference, if any.
    pub return_provenance: Option<CanonicalProvenance>,
    /// Normalized set of outlives constraints.
    pub outlives_constraints: Vec<CanonicalOutlivesConstraint>,
}

impl CanonicalLifetimeContract {
    /// Creates a new canonical lifetime contract with normalized constraints.
    pub fn new(
        return_provenance: Option<CanonicalProvenance>,
        mut outlives_constraints: Vec<CanonicalOutlivesConstraint>,
    ) -> Self {
        // Normalize constraints:
        // 1. Filter out reflexive constraints (outlives(x, x) is trivially satisfied)
        outlives_constraints.retain(|c| c.longer != c.shorter);
        // 2. Sort deterministically
        outlives_constraints.sort_unstable();
        // 3. Deduplicate
        outlives_constraints.dedup();

        Self {
            version: LIFETIME_RELATION_ABI_VERSION,
            return_provenance,
            outlives_constraints,
        }
    }

    /// Builds a canonical lifetime contract from AST declarations and source manager.
    /// Maps parameter names in `life_from(...)` and `where outlives(...)` to 0-indexed positions.
    pub fn from_ast(
        sig: &FnLifetimeSignature,
        params: &[luna_ast::DeclId],
        arena: &luna_ast::AstArena,
        source_manager: &luna_common::source::SourceManager,
    ) -> Result<Self, LifetimeError> {
        let find_param_index = |target_span: &Span| -> Result<u16, LifetimeError> {
            let target_name = extract_name_from_span(target_span, source_manager);
            for (i, &param_id) in params.iter().enumerate() {
                if let luna_ast::Decl::Param { name, .. } = &arena.decls[param_id.0 as usize] {
                    if extract_name_from_span(name, source_manager) == target_name {
                        return Ok(i as u16);
                    }
                }
            }
            Err(LifetimeError::UnresolvedLifetime {
                name: target_name,
                span: *target_span,
            })
        };

        // 1. Resolve return provenance
        let return_provenance = match &sig.provenance {
            Some(LifetimeExpr::Provenance(span)) => {
                let idx = find_param_index(span)?;
                Some(CanonicalProvenance::Param(idx))
            }
            Some(LifetimeExpr::ProvenanceSet(spans)) => {
                let mut indices = Vec::with_capacity(spans.len());
                for span in spans {
                    indices.push(find_param_index(span)?);
                }
                CanonicalProvenance::from_indices(indices)
            }
            None => None,
        };

        // 2. Resolve outlives constraints: where outlives(longer, shorter)
        // Semantics: lifetime(longer) >= lifetime(shorter)
        let mut outlives_constraints = Vec::with_capacity(sig.constraints.len());
        for constraint in &sig.constraints {
            let longer = find_param_index(&constraint.first)?;
            let shorter = find_param_index(&constraint.second)?;
            outlives_constraints.push(CanonicalOutlivesConstraint::new(longer, shorter));
        }

        Ok(Self::new(return_provenance, outlives_constraints))
    }

    /// Returns true if this signature has no lifetime relations or constraints.
    pub fn is_empty(&self) -> bool {
        self.return_provenance.is_none() && self.outlives_constraints.is_empty()
    }

    /// Returns the transitive closure of outlives relationships (longer, shorter)
    /// where (a, b) means parameter `a` outlives parameter `b` (lifetime(a) >= lifetime(b)).
    pub fn transitive_outlives(&self) -> std::collections::HashSet<(u16, u16)> {
        use std::collections::HashSet;
        let mut closure = HashSet::new();

        for c in &self.outlives_constraints {
            closure.insert((c.longer, c.shorter));
            closure.insert((c.longer, c.longer));
            closure.insert((c.shorter, c.shorter));
        }

        let mut changed = true;
        while changed {
            changed = false;
            let current: Vec<(u16, u16)> = closure.iter().copied().collect();
            for &(a, b) in &current {
                for &(c, d) in &current {
                    if b == c && closure.insert((a, d)) {
                        changed = true;
                    }
                }
            }
        }

        closure
    }

    /// Checks whether parameter `longer` outlives parameter `shorter` under this contract.
    pub fn outlives_holds(&self, longer: u16, shorter: u16) -> bool {
        if longer == shorter {
            return true;
        }
        self.transitive_outlives().contains(&(longer, shorter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provenance_canonicalization() {
        // {a} ≡ a
        let p1 = Provenance::ProvenanceSet(vec![Provenance::Input(SymbolId(1))]);
        let p1_canon = p1.canonicalize();
        assert!(matches!(p1_canon, Provenance::Input(SymbolId(1))));
    }

    #[test]
    fn test_solver_simple() {
        let mut solver = LifetimeSolver::new();
        solver.register_lifetime("a", LifetimeIdent(0));
        solver.register_lifetime("b", LifetimeIdent(1));

        // Add constraint: 'b ≤ 'a
        solver.add_constraint(LifetimeConstraintExpr {
            longer: LifetimeIdent(1),
            shorter: LifetimeIdent(0),
        });

        match solver.solve() {
            SolveResult::Sat(_) => {},
            other => panic!("expected Sat, got {:?}", other),
        }
    }

    #[test]
    fn test_solver_cycle() {
        let mut solver = LifetimeSolver::new();
        solver.register_lifetime("a", LifetimeIdent(0));
        solver.register_lifetime("b", LifetimeIdent(1));

        // Add mutual constraints: 'a ≤ 'b and 'b ≤ 'a
        // This forms an SCC but should be satisfiable (a = b)
        solver.add_constraint(LifetimeConstraintExpr {
            longer: LifetimeIdent(1),
            shorter: LifetimeIdent(0),
        });
        solver.add_constraint(LifetimeConstraintExpr {
            longer: LifetimeIdent(0),
            shorter: LifetimeIdent(1),
        });

        match solver.solve() {
            SolveResult::Sat(assignment) => {
                // Both should be equal (same SCC)
                assert_eq!(assignment.lifetimes.get(&LifetimeIdent(0)), assignment.lifetimes.get(&LifetimeIdent(1)));
            }
            other => panic!("expected Sat for mutual constraints, got {:?}", other),
        }
    }

    #[test]
    fn test_canonical_provenance_canonicalization() {
        // Single parameter
        assert_eq!(
            CanonicalProvenance::from_indices(vec![2]),
            Some(CanonicalProvenance::Param(2))
        );
        // Duplicate single parameter collapses to Param(i)
        assert_eq!(
            CanonicalProvenance::from_indices(vec![1, 1, 1]),
            Some(CanonicalProvenance::Param(1))
        );
        // Unordered and duplicate set is sorted and deduplicated
        assert_eq!(
            CanonicalProvenance::from_indices(vec![3, 1, 2, 1, 3]),
            Some(CanonicalProvenance::ParamSet(vec![1, 2, 3]))
        );
        // Empty produces None
        assert_eq!(CanonicalProvenance::from_indices(vec![]), None);
    }

    #[test]
    fn test_canonical_lifetime_contract_normalization() {
        let constraints = vec![
            CanonicalOutlivesConstraint::new(1, 0),
            CanonicalOutlivesConstraint::new(0, 0), // Reflexive, should be pruned
            CanonicalOutlivesConstraint::new(2, 1),
            CanonicalOutlivesConstraint::new(1, 0), // Duplicate, should be deduped
        ];

        let contract = CanonicalLifetimeContract::new(
            Some(CanonicalProvenance::ParamSet(vec![0, 1])),
            constraints,
        );

        assert_eq!(contract.version, LIFETIME_RELATION_ABI_VERSION);
        assert_eq!(
            contract.return_provenance,
            Some(CanonicalProvenance::ParamSet(vec![0, 1]))
        );
        // Constraints must be sorted and have duplicates and reflexives removed
        assert_eq!(
            contract.outlives_constraints,
            vec![
                CanonicalOutlivesConstraint::new(1, 0),
                CanonicalOutlivesConstraint::new(2, 1),
            ]
        );
    }

    #[test]
    fn test_canonical_transitive_outlives() {
        // Contract: 2 >= 1, 1 >= 0
        let contract = CanonicalLifetimeContract::new(
            None,
            vec![
                CanonicalOutlivesConstraint::new(2, 1),
                CanonicalOutlivesConstraint::new(1, 0),
            ],
        );

        assert!(contract.outlives_holds(2, 1));
        assert!(contract.outlives_holds(1, 0));
        assert!(contract.outlives_holds(2, 0)); // Transitive: 2 >= 0
        assert!(contract.outlives_holds(2, 2)); // Reflexive
        assert!(contract.outlives_holds(1, 1));
        assert!(contract.outlives_holds(0, 0));

        // Inverted: 0 >= 2 or 1 >= 2 must be false
        assert!(!contract.outlives_holds(0, 1));
        assert!(!contract.outlives_holds(0, 2));
        assert!(!contract.outlives_holds(1, 2));
    }
}
