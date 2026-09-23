//! CFG Point and Boundary Fact Model (REGION-01D-A).
//!
//! # Architectural Role & Invariants
//! This module defines the canonical fact representation that connects symbolic
//! regions to control-flow execution without depending directly on `luna-mvir` or
//! duplicating CFG structures.
//!
//! ### Core Design Invariants:
//! 1. **Dual-Fact Separation**:
//!    - **Point Facts** ([`RegionUseRequirement`]): Record *where* a reference is used
//!      ($p \in \text{RequiredPoints}(R)$).
//!    - **Boundary Facts** ([`RegionBoundaryExit`]): Record *which dynamic region instances*
//!      terminate upon crossing a CFG edge (e.g. `continue`, back-edge, `break`).
//! 2. **One Edge $\to N$ Terminated Regions**:
//!    A single CFG edge (such as `return` from a nested loop) can terminate multiple
//!    nested dynamic instances simultaneously. Facts are normalized per `(edge, region)` pair.
//! 3. **Symbolic vs. Concrete Bound Separation**:
//!    - `StaticBound` (01C): Symbolic negative relation $\neg(A \succeq B)$.
//!    - [`PointDomainBound`] (01D): Concrete CFG-domain restriction ($p \in/\notin R$).
//! 4. **Contract Region Non-Realizability**:
//!    Contract and External regions are abstract API variables; they cannot be directly
//!    assigned local CFG points without callsite instantiation.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use luna_common::Span;

use crate::region::id::RegionId;
use crate::region::kind::RegionKind;

/// An opaque identifier representing a program point within a control-flow graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProgramPointId(pub u32);

impl ProgramPointId {
    /// Returns the 0-based index of this program point.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Constructs a `ProgramPointId` from a raw index.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// A canonical position in a control-flow graph (basic block + instruction/terminator position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProgramPoint {
    pub block: u32,
    pub position: u32,
}

impl ProgramPoint {
    #[inline]
    pub const fn new(block: u32, position: u32) -> Self {
        Self { block, position }
    }
}

/// An opaque identifier representing a directed CFG transition edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CfgEdgeId(pub u32);

impl CfgEdgeId {
    /// Returns the 0-based index of this CFG edge.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Constructs a `CfgEdgeId` from a raw index.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// A directed CFG edge from a predecessor basic block to a successor basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CfgEdge {
    pub from_block: u32,
    pub to_block: u32,
}

impl CfgEdge {
    #[inline]
    pub const fn new(from_block: u32, to_block: u32) -> Self {
        Self { from_block, to_block }
    }
}

/// A fact asserting that a region must be valid at a specific program point.
///
/// Corresponds to $p \in \text{RequiredPoints}(R)$ derived from reference usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionUseRequirement {
    /// The region whose validity is required.
    pub region: RegionId,
    /// The program point where the reference is active/used.
    pub point: ProgramPointId,
    /// Source span associated with this use requirement for diagnostics.
    pub span: Option<Span>,
}

impl RegionUseRequirement {
    pub fn new(region: RegionId, point: ProgramPointId, span: Option<Span>) -> Self {
        Self { region, point, span }
    }
}

/// A fact asserting that crossing a directed CFG edge terminates a dynamic region instance.
///
/// # Invariant
/// Crossing edge `edge` terminates region instance `region`.
/// A reference or carrier whose required validity depends on `region` cannot cross this edge.
/// One CFG edge can terminate multiple region instances simultaneously (e.g. nested loop break).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegionBoundaryExit {
    pub edge: CfgEdgeId,
    pub region: RegionId,
}

impl RegionBoundaryExit {
    pub fn new(edge: CfgEdgeId, region: RegionId) -> Self {
        Self { edge, region }
    }
}

/// Concrete CFG-domain restriction for a fixed region (e.g. lexical block or program).
///
/// Distinguishes concrete CFG point bounds (01D) from symbolic negative relations (01C `StaticBound`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointDomainBound {
    pub region: RegionId,
    pub allowed_points: HashSet<ProgramPointId>,
}

impl PointDomainBound {
    pub fn new(region: RegionId, allowed_points: HashSet<ProgramPointId>) -> Self {
        Self { region, allowed_points }
    }
}

/// Determines whether a region kind is point-realizable in the local CFG.
///
/// - `Program`, `Lexical`, `Iteration`, and `Inference` are point-realizable.
/// - `Contract` and `External` are abstract API parameters that must be instantiated
///   at callsites before concrete point realization.
pub fn is_point_realizable(kind: &RegionKind) -> bool {
    match kind {
        RegionKind::Program
        | RegionKind::Lexical { .. }
        | RegionKind::Iteration { .. }
        | RegionKind::Inference { .. } => true,
        RegionKind::Contract | RegionKind::External { .. } => false,
    }
}

/// Container for all CFG-derived point and boundary facts feeding into Region Realization.
#[derive(Debug, Clone, Default)]
pub struct RealizationFacts {
    requirements: Vec<RegionUseRequirement>,
    boundary_exits: Vec<RegionBoundaryExit>,
    domain_bounds: HashMap<RegionId, HashSet<ProgramPointId>>,
    edge_to_exits: HashMap<CfgEdgeId, Vec<RegionId>>,
    region_to_requirements: HashMap<RegionId, Vec<ProgramPointId>>,
}

impl RealizationFacts {
    /// Creates a new, empty fact container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a reference use requirement: region `r` must be valid at point `p`.
    pub fn add_use_requirement(
        &mut self,
        region: RegionId,
        point: ProgramPointId,
        span: Option<Span>,
    ) {
        self.requirements.push(RegionUseRequirement::new(region, point, span));
        self.region_to_requirements.entry(region).or_default().push(point);
    }

    /// Records that crossing `edge` terminates the dynamic instance of `region`.
    ///
    /// Supports 1 edge terminating multiple regions (e.g. nested loop exit).
    pub fn add_boundary_exit(&mut self, edge: CfgEdgeId, region: RegionId) {
        let exit = RegionBoundaryExit::new(edge, region);
        if !self.boundary_exits.contains(&exit) {
            self.boundary_exits.push(exit);
            self.edge_to_exits.entry(edge).or_default().push(region);
        }
    }

    /// Sets the allowed concrete CFG point domain for a fixed region (e.g. lexical scope).
    pub fn set_allowed_domain(&mut self, region: RegionId, points: HashSet<ProgramPointId>) {
        self.domain_bounds.insert(region, points);
    }

    /// Returns all registered use requirements.
    pub fn requirements(&self) -> &[RegionUseRequirement] {
        &self.requirements
    }

    /// Returns all registered boundary exits.
    pub fn boundary_exits(&self) -> &[RegionBoundaryExit] {
        &self.boundary_exits
    }

    /// Returns the slice of terminated regions when crossing the specified CFG edge, if any.
    pub fn terminated_regions_at_edge(&self, edge: CfgEdgeId) -> Option<&[RegionId]> {
        self.edge_to_exits.get(&edge).map(|v| v.as_slice())
    }

    /// Returns direct required points seeded for a region.
    pub fn direct_required_points(&self, region: RegionId) -> Option<&[ProgramPointId]> {
        self.region_to_requirements.get(&region).map(|v| v.as_slice())
    }

    /// Returns the allowed point domain for a fixed region, if defined.
    pub fn allowed_domain(&self, region: RegionId) -> Option<&HashSet<ProgramPointId>> {
        self.domain_bounds.get(&region)
    }
}
