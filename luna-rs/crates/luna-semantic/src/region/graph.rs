//! RegionGraph data structure and structural operations for Luna Region IR.

use std::collections::{HashMap, HashSet};
use luna_common::Span;
use serde::{Deserialize, Serialize};

use super::constraint::{ConstraintKind, ConstraintOrigin, RegionConstraint};
use super::id::{ConstraintId, RegionId};
use super::kind::{RegionKind, RegionNode};

/// Errors that can occur during graph construction or structural validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionGraphError {
    /// Attempted to reference a `RegionId` that does not exist in this graph.
    UnknownRegion(RegionId),

    /// Attempted to add more than one canonical Program region.
    DuplicateProgramRegion,
}

impl std::fmt::Display for RegionGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownRegion(id) => write!(f, "Unknown RegionId({})", id.index()),
            Self::DuplicateProgramRegion => {
                write!(f, "Cannot add duplicate Program region; graph already contains a canonical Program region")
            }
        }
    }
}

impl std::error::Error for RegionGraphError {}

/// The core Region Graph representation in the Luna compiler.
///
/// # Architecture & Boundaries
/// `RegionGraph` owns the region nodes and all recorded constraints.
/// Adjacency is a derived/cached index, not the primary source of truth.
///
/// ## REGION-01A Invariants:
/// - **Program Canonical Root**: Exactly one Program region per graph, created on initialization.
/// - **Direct Edge Queries**: Methods like `has_direct_outlives` strictly check direct edges.
///   Transitive closure, contradiction detection, and escape diagnostics belong to REGION-01C.
/// - **Reflexivity & Cycles**: Self-edges (`R ⪰ R`) and cycles (`Ra ⪰ Rb ∧ Rb ⪰ Ra`) are
///   structurally legal and accepted without error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionGraph {
    /// The unique canonical Program region identifier.
    program: RegionId,

    /// All region nodes registered in this graph.
    nodes: Vec<RegionNode>,

    /// Complete sequence of all inserted constraints, preserving order, origin, and spans.
    constraints: Vec<RegionConstraint>,

    /// Derived, deduplicated direct outlives index (`sup -> set of subs`).
    outlives_adjacency: HashMap<RegionId, HashSet<RegionId>>,
}

impl RegionGraph {
    /// Constructs a new `RegionGraph` with the canonical `Program` root region.
    pub fn new() -> Self {
        let program_id = RegionId::from_raw(0);
        let program_node = RegionNode::new(program_id, RegionKind::Program);

        Self {
            program: program_id,
            nodes: vec![program_node],
            constraints: Vec::new(),
            outlives_adjacency: HashMap::new(),
        }
    }

    /// Returns the canonical `Program` region identifier.
    #[inline]
    pub fn program_region(&self) -> RegionId {
        self.program
    }

    /// Checks whether the given `RegionId` exists in this graph.
    #[inline]
    pub fn contains_region(&self, id: RegionId) -> bool {
        (id.index() as usize) < self.nodes.len()
    }

    /// Retrieves a region node by its `RegionId`.
    #[inline]
    pub fn get_node(&self, id: RegionId) -> Option<&RegionNode> {
        self.nodes.get(id.index() as usize)
    }

    /// Retrieves a recorded constraint by its `ConstraintId`.
    #[inline]
    pub fn get_constraint(&self, id: ConstraintId) -> Option<&RegionConstraint> {
        self.constraints.get(id.index() as usize)
    }

    /// Adds a new region node of the specified `RegionKind` to the graph.
    ///
    /// Returns `Err(RegionGraphError::DuplicateProgramRegion)` if attempting to add
    /// another `Program` region.
    pub fn add_region(&mut self, kind: RegionKind) -> Result<RegionId, RegionGraphError> {
        if kind == RegionKind::Program {
            return Err(RegionGraphError::DuplicateProgramRegion);
        }

        let raw_id = self.nodes.len() as u32;
        let id = RegionId::from_raw(raw_id);
        self.nodes.push(RegionNode::new(id, kind));
        Ok(id)
    }

    /// Adds a canonical constraint to the graph.
    ///
    /// Fails with `RegionGraphError::UnknownRegion` if any participating `RegionId`
    /// is not registered in this graph.
    ///
    /// Every constraint is appended to `constraints` preserving its `ConstraintOrigin`
    /// and `Option<Span>`. Derived adjacency is deduplicated.
    pub fn add_constraint(
        &mut self,
        kind: ConstraintKind,
        origin: ConstraintOrigin,
        span: Option<Span>,
    ) -> Result<ConstraintId, RegionGraphError> {
        match kind {
            ConstraintKind::Outlives { sup, sub } => {
                if !self.contains_region(sup) {
                    return Err(RegionGraphError::UnknownRegion(sup));
                }
                if !self.contains_region(sub) {
                    return Err(RegionGraphError::UnknownRegion(sub));
                }

                // Update derived direct adjacency index (deduplicated)
                self.outlives_adjacency.entry(sup).or_default().insert(sub);

                // Preserve every constraint insertion and its metadata
                let constraint_id = ConstraintId::from_raw(self.constraints.len() as u32);
                self.constraints
                    .push(RegionConstraint::new(constraint_id, kind, origin, span));

                Ok(constraint_id)
            }
        }
    }

    /// Normalizes and adds an equality relation between two regions ($R_a = R_b$).
    ///
    /// Equality is desugared to bidirectional outlives constraints:
    /// $R_a \succeq R_b$ and $R_b \succeq R_a$.
    pub fn add_equality(
        &mut self,
        a: RegionId,
        b: RegionId,
        origin: ConstraintOrigin,
        span: Option<Span>,
    ) -> Result<(ConstraintId, ConstraintId), RegionGraphError> {
        let c1 = self.add_constraint(ConstraintKind::Outlives { sup: a, sub: b }, origin, span)?;
        let c2 = self.add_constraint(ConstraintKind::Outlives { sup: b, sub: a }, origin, span)?;
        Ok((c1, c2))
    }

    /// Answers whether a direct outlives edge ($sup \succeq sub$) has been inserted into the graph.
    ///
    /// # Important Boundary
    /// This strictly queries the direct edge cache. It does **not** evaluate transitive
    /// reachability or solver proofs (which is the domain of REGION-01C).
    pub fn has_direct_outlives(&self, sup: RegionId, sub: RegionId) -> bool {
        self.outlives_adjacency
            .get(&sup)
            .map_or(false, |subs| subs.contains(&sub))
    }

    /// Returns direct outlives targets for a given region, if any.
    pub fn direct_outlives_targets(&self, sup: RegionId) -> Option<&HashSet<RegionId>> {
        self.outlives_adjacency.get(&sup)
    }

    /// Returns a slice of all registered region nodes.
    #[inline]
    pub fn nodes(&self) -> &[RegionNode] {
        &self.nodes
    }

    /// Returns a slice of all recorded constraints in insertion order.
    #[inline]
    pub fn constraints(&self) -> &[RegionConstraint] {
        &self.constraints
    }
}

impl Default for RegionGraph {
    fn default() -> Self {
        Self::new()
    }
}
