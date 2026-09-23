//! CFG Region Realization & Borrow Integration (REGION-01D).
//!
//! Submodules:
//! - [`facts`]: CFG point and boundary fact model (01D-A).
//! - [`cfg`]: Abstract CFG navigation trait & in-memory test view (01D-B).
//! - [`engine`]: Point requirement propagation, domain & boundary validation (01D-B).

pub mod cfg;
pub mod engine;
pub mod facts;

pub use cfg::{InMemoryCfgView, RegionCfgView};
pub use engine::{realize_regions, RealizationError, RegionRealization};
pub use facts::{
    is_point_realizable, CfgEdge, CfgEdgeId, PointDomainBound, ProgramPoint, ProgramPointId,
    RealizationFacts, RegionBoundaryExit, RegionUseRequirement,
};
