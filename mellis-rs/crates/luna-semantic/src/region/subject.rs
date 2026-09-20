//! Semantic lifetime subjects and region bindings (REGION-01B).
//!
//! Replaces ad-hoc string and positional lookups with strongly-typed semantic targets.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::error::RegionGenError;
use super::graph::RegionGraph;
use super::id::RegionId;
use super::kind::RegionKind;

/// The base / root target of a lifetime subject in an API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LifetimeSubjectRoot {
    /// Function parameter by 0-based index.
    Param(u16),
    /// The implicit or explicit `self` receiver.
    SelfVal,
    /// The returned reference or value.
    Return,
}

/// A resolved semantic subject capable of participating in lifetime contracts.
///
/// Distinguishes direct roots (`Param`, `SelfVal`, `Return`) from static field projections
/// (`Param(0).value`, `Self.inner.field`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LifetimeSubject {
    /// A root entity without projections.
    Root(LifetimeSubjectRoot),

    /// A static field projection originating from a root entity.
    ///
    /// # Scope
    /// REGION-01B supports resolved static field projections.
    /// Index, deref, and path-dependent projections remain outside this phase.
    Projection {
        base: LifetimeSubjectRoot,
        field_path: Vec<u32>,
    },
}

impl LifetimeSubject {
    /// Constructs a parameter root subject.
    pub const fn param(index: u16) -> Self {
        Self::Root(LifetimeSubjectRoot::Param(index))
    }

    /// Constructs a `self` root subject.
    pub const fn self_val() -> Self {
        Self::Root(LifetimeSubjectRoot::SelfVal)
    }

    /// Constructs a `return` root subject.
    pub const fn return_val() -> Self {
        Self::Root(LifetimeSubjectRoot::Return)
    }

    /// Constructs a single-level field projection subject.
    pub fn field(base: LifetimeSubjectRoot, field_idx: u32) -> Self {
        Self::Projection {
            base,
            field_path: vec![field_idx],
        }
    }

    /// Constructs a multi-level field projection subject.
    pub fn projection(base: LifetimeSubjectRoot, field_path: Vec<u32>) -> Self {
        if field_path.is_empty() {
            Self::Root(base)
        } else {
            Self::Projection { base, field_path }
        }
    }
}

/// A mapping from strongly-typed `LifetimeSubject`s to graph-local `RegionId`s.
#[derive(Debug, Clone, Default)]
pub struct LifetimeRegionBindings {
    bindings: HashMap<LifetimeSubject, RegionId>,
}

impl LifetimeRegionBindings {
    /// Creates a new, empty set of bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a semantic subject to a specific `RegionId`.
    ///
    /// Returns `Err(RegionGenError::DuplicateLifetimeBinding)` if already bound.
    pub fn bind(
        &mut self,
        subject: LifetimeSubject,
        region: RegionId,
    ) -> Result<(), RegionGenError> {
        if self.bindings.contains_key(&subject) {
            return Err(RegionGenError::DuplicateLifetimeBinding(subject));
        }
        self.bindings.insert(subject, region);
        Ok(())
    }

    /// Binds a semantic subject, overwriting any previous binding.
    pub fn insert(&mut self, subject: LifetimeSubject, region: RegionId) {
        self.bindings.insert(subject, region);
    }

    /// Queries the bound `RegionId` for a subject, if present.
    pub fn get(&self, subject: &LifetimeSubject) -> Option<RegionId> {
        self.bindings.get(subject).copied()
    }

    /// Retrieves the bound `RegionId` or returns `Err(RegionGenError::UnboundLifetimeSubject)`.
    pub fn require(&self, subject: &LifetimeSubject) -> Result<RegionId, RegionGenError> {
        self.get(subject)
            .ok_or_else(|| RegionGenError::UnboundLifetimeSubject(subject.clone()))
    }

    /// Helper to instantiate fresh `RegionKind::Contract` regions for a function signature.
    pub fn instantiate_function_bindings(
        graph: &mut RegionGraph,
        param_count: usize,
        has_return: bool,
        has_self: bool,
    ) -> Result<Self, RegionGenError> {
        let mut bindings = Self::new();

        if has_self {
            let rid = graph.add_region(RegionKind::Contract)?;
            bindings.bind(LifetimeSubject::self_val(), rid)?;
        }

        for i in 0..param_count {
            let rid = graph.add_region(RegionKind::Contract)?;
            bindings.bind(LifetimeSubject::param(i as u16), rid)?;
        }

        if has_return {
            let rid = graph.add_region(RegionKind::Contract)?;
            bindings.bind(LifetimeSubject::return_val(), rid)?;
        }

        Ok(bindings)
    }
}
