//! Contract Constraint Generator (REGION-01B).
//!
//! Translates high-level API boundary contracts into canonical `Outlives` constraints
//! in the `RegionGraph`.

use luna_common::Span;

use super::constraint::{
    ConstraintBoundary, ConstraintKind, ConstraintOrigin, ConstraintRule, ConstraintTransport,
};
use super::error::RegionGenError;
use super::graph::RegionGraph;
use super::id::ConstraintId;
use super::subject::{LifetimeRegionBindings, LifetimeSubject};

/// A resolved function lifetime contract operating on strongly-typed `LifetimeSubject`s.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedFunctionContract {
    /// Explicit return provenance sources (`life_from(...)`).
    pub return_sources: Vec<LifetimeSubject>,

    /// Explicit relation constraints (`requires life(sup) >= life(sub)`).
    pub requires: Vec<(LifetimeSubject, LifetimeSubject)>,
}

impl ResolvedFunctionContract {
    /// Creates an empty resolved contract.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a `life_from` source.
    pub fn with_return_source(mut self, source: LifetimeSubject) -> Self {
        self.return_sources.push(source);
        self
    }

    /// Adds an explicit `requires life(sup) >= life(sub)` constraint.
    pub fn with_outlives(mut self, sup: LifetimeSubject, sub: LifetimeSubject) -> Self {
        self.requires.push((sup, sub));
        self
    }

    /// Answers whether this contract explicitly binds the return lifetime.
    pub fn has_explicit_return_binding(&self) -> bool {
        if !self.return_sources.is_empty() {
            return true;
        }
        self.requires
            .iter()
            .any(|(_, sub)| matches!(sub, LifetimeSubject::Root(super::subject::LifetimeSubjectRoot::Return)))
    }

    /// Constructs a resolved contract containing ONLY input preconditions from a canonical contract.
    ///
    /// # Architectural Invariant
    /// Return guarantees (`life_from`, return outlives) are strictly excluded to avoid circular proof premises.
    pub fn from_canonical_preconditions(contract: &crate::CanonicalLifetimeContract) -> Self {
        let mut resolved = Self::new();
        for (longer, shorter) in contract.input_preconditions() {
            resolved = resolved.with_outlives(longer, shorter);
        }
        resolved
    }
}

/// The generator responsible for lowering API contracts to canonical region constraints.
#[derive(Debug, Default)]
pub struct ContractConstraintGenerator;

impl ContractConstraintGenerator {
    /// Creates a new `ContractConstraintGenerator`.
    pub fn new() -> Self {
        Self
    }

    /// Lowers a resolved function contract into the `RegionGraph`.
    ///
    /// # Architectural Invariants:
    /// - **Precedence**: An explicit return-lifetime contract suppresses Return LLE.
    ///   Unrelated requires constraints do not suppress LLE.
    /// - **FFI Exception**: `is_extern == true` NEVER applies LLE (*no inference across FFI*).
    /// - **Intersection `life_from`**: Multiple sources yield multiple parallel outlives edges.
    pub fn lower_function_contract(
        &self,
        graph: &mut RegionGraph,
        bindings: &LifetimeRegionBindings,
        contract: &ResolvedFunctionContract,
        is_extern: bool,
        input_ref_subjects: &[LifetimeSubject],
        has_return_ref: bool,
        transport: ConstraintTransport,
        span: Option<Span>,
    ) -> Result<Vec<ConstraintId>, RegionGenError> {
        let mut generated = Vec::new();
        let boundary = if is_extern {
            ConstraintBoundary::Ffi
        } else {
            ConstraintBoundary::Normal
        };

        // 1. Lower explicit life_from sources
        if !contract.return_sources.is_empty() {
            let return_sub = LifetimeSubject::return_val();
            let return_region = bindings.require(&return_sub)?;

            for source_subject in &contract.return_sources {
                let source_region = bindings.require(source_subject)?;
                // Invariant: Source outlives Return (R_source ⪰ R_return)
                let cid = graph.add_constraint(
                    ConstraintKind::Outlives {
                        sup: source_region,
                        sub: return_region,
                    },
                    ConstraintOrigin::life_from(transport, boundary),
                    span,
                )?;
                generated.push(cid);
            }
        }

        // 2. Lower explicit requires clauses (requires life(sup) >= life(sub))
        for (sup_subject, sub_subject) in &contract.requires {
            let sup_region = bindings.require(sup_subject)?;
            let sub_region = bindings.require(sub_subject)?;

            // Invariant: sup outlives sub (R_sup ⪰ R_sub)
            let cid = graph.add_constraint(
                ConstraintKind::Outlives {
                    sup: sup_region,
                    sub: sub_region,
                },
                ConstraintOrigin::explicit_contract(transport, boundary),
                span,
            )?;
            generated.push(cid);
        }

        // 3. Evaluate Luna Lifetime Elision (LLE)
        // Precedence Rule: explicit return contract suppresses return elision.
        // FFI Rule: extern fn NEVER applies LLE.
        let return_already_constrained = contract.has_explicit_return_binding();

        if !is_extern && !return_already_constrained && has_return_ref && input_ref_subjects.len() == 1 {
            let input_subject = &input_ref_subjects[0];
            let input_region = bindings.require(input_subject)?;
            let return_region = bindings.require(&LifetimeSubject::return_val())?;

            // Elision: single input reference outlives return reference
            let cid = graph.add_constraint(
                ConstraintKind::Outlives {
                    sup: input_region,
                    sub: return_region,
                },
                ConstraintOrigin::elision(),
                span,
            )?;
            generated.push(cid);
        }

        Ok(generated)
    }

    /// Lowers a struct/type lifetime contract into the `RegionGraph`.
    ///
    /// # Invariant:
    /// Stored value must outlive the struct instance: $R_{field} \succeq R_{self}$.
    pub fn lower_type_contract(
        &self,
        graph: &mut RegionGraph,
        bindings: &LifetimeRegionBindings,
        field_subjects: &[LifetimeSubject],
        self_subject: &LifetimeSubject,
        transport: ConstraintTransport,
        span: Option<Span>,
    ) -> Result<Vec<ConstraintId>, RegionGenError> {
        let mut generated = Vec::new();
        let self_region = bindings.require(self_subject)?;

        for field_subject in field_subjects {
            let field_region = bindings.require(field_subject)?;

            // Invariant: Field value outlives self instance (R_field ⪰ R_self)
            let cid = graph.add_constraint(
                ConstraintKind::Outlives {
                    sup: field_region,
                    sub: self_region,
                },
                ConstraintOrigin::explicit_contract(transport, ConstraintBoundary::Normal),
                span,
            )?;
            generated.push(cid);
        }

        Ok(generated)
    }

    /// Instantiates a post-elision canonical contract (e.g. from `.llib` metadata).
    ///
    /// Preserves semantic contract rules while generating fresh graph-local regions.
    pub fn lower_canonical_contract(
        &self,
        graph: &mut RegionGraph,
        bindings: &LifetimeRegionBindings,
        requires: &[(LifetimeSubject, LifetimeSubject, ConstraintRule)],
        boundary: ConstraintBoundary,
        span: Option<Span>,
    ) -> Result<Vec<ConstraintId>, RegionGenError> {
        let mut generated = Vec::new();

        for (sup_sub, sub_sub, rule) in requires {
            let sup_region = bindings.require(sup_sub)?;
            let sub_region = bindings.require(sub_sub)?;

            let cid = graph.add_constraint(
                ConstraintKind::Outlives {
                    sup: sup_region,
                    sub: sub_region,
                },
                ConstraintOrigin::new(*rule, ConstraintTransport::Artifact, boundary),
                span,
            )?;
            generated.push(cid);
        }

        Ok(generated)
    }
}
