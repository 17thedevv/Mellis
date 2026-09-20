//! Region kinds and nodes for Luna Region IR.

use serde::{Deserialize, Serialize};
use super::id::RegionId;

/// Categorizes the validity boundary represented by a Region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionKind {
    /// Lexical scope boundary (block, function body).
    Lexical { scope: u32 },

    /// Dynamic loop iteration family boundary.
    ///
    /// # Invariant: Static Place != Dynamic Lifetime Instance
    /// `Iteration` represents a static iteration family boundary: each dynamic
    /// traversal of the loop instantiates a distinct lifetime instance at runtime.
    /// The static compiler IR never enumerates concrete runtime iteration indices.
    Iteration { loop_id: u32 },

    /// Abstract public/API region variable (parameters, return, self, struct fields in contracts).
    ///
    /// Distinct variables are differentiated by their unique `RegionId`.
    Contract,

    /// Locally inferred validity variable (temporary reference lifetime in function bodies).
    ///
    /// Enables non-lexical lifetimes (NLL) without regressing to whole-block lexical scopes.
    Inference { variable: u32 },

    /// Abstract boundary for external contracts (e.g. FFI, cross-artifact).
    ///
    /// External represents an abstract contract validity domain, not the physical
    /// FFI call itself and not external storage.
    External { boundary: u32 },



    /// Canonical top-level Program lifetime.
    ///
    /// Every valid `RegionGraph` has exactly one Program region, which serves
    /// as the maximum validity domain.
    Program,
}

/// A node in the `RegionGraph`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionNode {
    /// Unique identifier for this region within the graph.
    pub id: RegionId,
    /// The structural kind of this region.
    pub kind: RegionKind,
}

impl RegionNode {
    /// Creates a new region node.
    pub fn new(id: RegionId, kind: RegionKind) -> Self {
        Self { id, kind }
    }
}
