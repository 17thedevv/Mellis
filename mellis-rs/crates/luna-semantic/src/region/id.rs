//! Identifiers for the Luna Region IR.
//!
//! # Graph-Local Invariant
//! `RegionId` and `ConstraintId` are 0-based opaque monotonic indices emitted
//! solely by a `RegionGraph`. Cross-graph IDs are invalid by API contract.
//! Identifiers are never reused or deleted within the graph.

use serde::{Deserialize, Serialize};

/// An opaque identifier representing a region node within a `RegionGraph`.
///
/// Region IDs are stable for the lifetime of a `RegionGraph` and are strictly graph-local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegionId(u32);

impl RegionId {
    /// Returns the 0-based raw index of this region.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Raw factory to construct a `RegionId`.
    ///
    /// Marked doc(hidden) for internal use and testing. Production code should
    /// only obtain RegionIds through `RegionGraph`.
    #[doc(hidden)]
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// An opaque identifier representing a constraint within a `RegionGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConstraintId(u32);

impl ConstraintId {
    /// Returns the 0-based raw index of this constraint.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Internal factory to construct a `ConstraintId`.
    #[inline]
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}
