//! Body Constraint Generator (REGION-01B).
//!
//! Translates body-level semantic facts (lexical scopes, loop iteration boundaries,
//! reference creations, closure captures) into canonical `Outlives` constraints in the `RegionGraph`.

use luna_common::Span;

use super::constraint::{ConstraintKind, ConstraintOrigin};
use super::error::RegionGenError;
use super::graph::RegionGraph;
use super::id::{ConstraintId, RegionId};
use super::kind::RegionKind;

/// Generator responsible for lowering body structural facts to region constraints.
#[derive(Debug, Default)]
pub struct BodyConstraintGenerator {
    next_inference_var: u32,
}

impl BodyConstraintGenerator {
    /// Creates a new `BodyConstraintGenerator`.
    pub fn new() -> Self {
        Self {
            next_inference_var: 0,
        }
    }

    /// Allocates a new lexical scope boundary region.
    pub fn generate_lexical_region(
        &self,
        graph: &mut RegionGraph,
        scope: u32,
    ) -> Result<RegionId, RegionGenError> {
        Ok(graph.add_region(RegionKind::Lexical { scope })?)
    }

    /// Allocates a new dynamic iteration family boundary region for a loop.
    ///
    /// # Invariant: Static Place != Dynamic Lifetime Instance
    /// Represents the loop family boundary, not concrete dynamic iteration indices.
    pub fn generate_loop_iteration_region(
        &self,
        graph: &mut RegionGraph,
        loop_id: u32,
    ) -> Result<RegionId, RegionGenError> {
        Ok(graph.add_region(RegionKind::Iteration { loop_id })?)
    }

    /// Allocates a fresh local inference region variable for a reference.
    ///
    /// # NLL Invariant
    /// Local reference variables use inference regions rather than whole lexical blocks.
    pub fn generate_inference_region(
        &mut self,
        graph: &mut RegionGraph,
    ) -> Result<RegionId, RegionGenError> {
        let var = self.next_inference_var;
        self.next_inference_var += 1;
        Ok(graph.add_region(RegionKind::Inference { variable: var })?)
    }

    /// Generates an outlives constraint for a reference borrowing operation (`&x` or `&rw x`).
    ///
    /// # Invariant: Place Identity != Lifetime Instance
    /// The referent's validity domain must outlive the reference's usage region:
    /// $$R_{referent} \succeq R_{ref}$$
    ///
    /// Both shared and mutable reference creations produce this exact relation.
    /// Exclusivity and loan conflicts are handled by the capability/borrow layer, not RegionGraph.
    pub fn generate_borrow_constraint(
        &self,
        graph: &mut RegionGraph,
        referent_validity_region: RegionId,
        ref_region: RegionId,
        span: Option<Span>,
    ) -> Result<ConstraintId, RegionGenError> {
        Ok(graph.add_constraint(
            ConstraintKind::Outlives {
                sup: referent_validity_region,
                sub: ref_region,
            },
            ConstraintOrigin::borrow(),
            span,
        )?)
    }

    /// Generates an outlives constraint for a closure environment capture.
    ///
    /// # Invariant: Captured Provenance Outlives Closure
    /// $$R_{captured} \succeq R_{closure}$$
    pub fn generate_closure_capture_constraint(
        &self,
        graph: &mut RegionGraph,
        captured_region: RegionId,
        closure_region: RegionId,
        span: Option<Span>,
    ) -> Result<ConstraintId, RegionGenError> {
        Ok(graph.add_constraint(
            ConstraintKind::Outlives {
                sup: captured_region,
                sub: closure_region,
            },
            ConstraintOrigin::closure_capture(),
            span,
        )?)
    }

    /// Generates a structural outlives constraint for nested lexical scopes.
    ///
    /// # Structural Invariant
    /// The outer lexical scope contains and outlives the inner lexical scope:
    /// $$R_{outer} \succeq R_{inner}$$
    pub fn generate_lexical_nesting_constraint(
        &self,
        graph: &mut RegionGraph,
        outer_scope_region: RegionId,
        inner_scope_region: RegionId,
        span: Option<Span>,
    ) -> Result<ConstraintId, RegionGenError> {
        Ok(graph.add_constraint(
            ConstraintKind::Outlives {
                sup: outer_scope_region,
                sub: inner_scope_region,
            },
            ConstraintOrigin::new(
                super::constraint::ConstraintRule::Structural,
                super::constraint::ConstraintTransport::Synthesized,
                super::constraint::ConstraintBoundary::Normal,
            ),
            span,
        )?)
    }

    /// Generates a structural outlives constraint for a loop within its enclosing scope.
    ///
    /// # Structural Invariant
    /// The enclosing scope outlives the dynamic iteration family:
    /// $$R_{enclosing} \succeq R_{iteration}$$
    pub fn generate_loop_nesting_constraint(
        &self,
        graph: &mut RegionGraph,
        enclosing_scope_region: RegionId,
        iteration_region: RegionId,
        span: Option<Span>,
    ) -> Result<ConstraintId, RegionGenError> {
        Ok(graph.add_constraint(
            ConstraintKind::Outlives {
                sup: enclosing_scope_region,
                sub: iteration_region,
            },
            ConstraintOrigin::new(
                super::constraint::ConstraintRule::Structural,
                super::constraint::ConstraintTransport::Synthesized,
                super::constraint::ConstraintBoundary::Normal,
            ),
            span,
        )?)
    }

    /// Validates raw pointer operations under the negative rule.
    ///
    /// # Invariant: Raw Pointer Presence != Safe Lifetime
    /// Raw pointer creations (`*T`, `*rw T`), dereferences, or casts do NOT generate
    /// implicit safe region constraints in the graph.
    #[inline]
    pub fn observe_raw_pointer_operation(&self) {
        // Intentionally a no-op: raw pointers do not generate safe region constraints.
    }
}

