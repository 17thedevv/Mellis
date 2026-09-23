//! Formal Region IR Foundation (REGION-01A).
//!
//! This module defines the core data structures for Luna's formal region system:
//! - [`RegionId`] and [`ConstraintId`]: Opaque, graph-local monotonic identifiers.
//! - [`RegionKind`] and [`RegionNode`]: Categorization of region validity boundaries.
//! - [`ConstraintKind`], [`ConstraintOrigin`], and [`RegionConstraint`]: Canonical relations and diagnostic metadata.
//! - [`RegionGraph`]: Structural container managing nodes, constraints, and direct edge indexing.
//!
//! # Architectural Boundaries
//! - `luna-semantic::region` MUST NOT depend on `luna-borrowck`.
//! - REGION-01A does not perform solver semantics, transitive closure, or contradiction detection (deferred to REGION-01C).
//! - Binding and projection contracts are normalized before insertion into the canonical solver graph (REGION-01B).

pub mod body_gen;
pub mod constraint;
pub mod contract_gen;
pub mod error;
pub mod graph;
pub mod id;
pub mod kind;
pub mod realization;
pub mod solver;
pub mod subject;

pub use body_gen::BodyConstraintGenerator;
pub use constraint::{
    ConstraintBoundary, ConstraintKind, ConstraintOrigin, ConstraintRule, ConstraintTransport,
    RegionConstraint,
};
pub use contract_gen::{ContractConstraintGenerator, ResolvedFunctionContract};
pub use error::RegionGenError;
pub use graph::{RegionGraph, RegionGraphError};
pub use id::{ConstraintId, RegionId};
pub use kind::{RegionKind, RegionNode};
pub use realization::{
    is_point_realizable, realize_regions, CfgEdge, CfgEdgeId, InMemoryCfgView, PointDomainBound,
    ProgramPoint, ProgramPointId, RealizationError, RealizationFacts, RegionBoundaryExit,
    RegionCfgView, RegionRealization, RegionUseRequirement,
};
pub use solver::{
    solve_region_graph, BoundViolation, EquivalenceClass, ProofPath, ProofStep, RegionSolution,
    SccId, StaticBound, StaticBoundKind,
};
pub use subject::{LifetimeRegionBindings, LifetimeSubject, LifetimeSubjectRoot};



