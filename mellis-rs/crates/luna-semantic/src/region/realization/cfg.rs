//! CFG View Trait & Adapters for Region Realization (REGION-01D-B).
//!
//! # Architectural Role
//! `RegionCfgView` is an abstract navigation interface over an external CFG representation
//! (such as MVIR CFG or Borrowck CFG). It ensures that `luna-semantic` does not depend
//! directly on `luna-mvir` or duplicate CFG data structures.

use std::collections::HashMap;
use super::facts::ProgramPointId;

/// An abstract, lightweight view for navigating control flow graph points.
pub trait RegionCfgView {
    /// Returns the direct successor program points of `point`.
    fn successors(&self, point: ProgramPointId) -> Vec<ProgramPointId>;

    /// Returns the direct predecessor program points of `point`.
    fn predecessors(&self, point: ProgramPointId) -> Vec<ProgramPointId>;
}

/// An in-memory CFG view useful for testing, synthesis, and lightweight adapters.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InMemoryCfgView {
    successors: HashMap<ProgramPointId, Vec<ProgramPointId>>,
    predecessors: HashMap<ProgramPointId, Vec<ProgramPointId>>,
}

impl InMemoryCfgView {
    /// Constructs a new, empty in-memory CFG view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a directed CFG transition edge between two program points.
    pub fn add_edge(&mut self, from: ProgramPointId, to: ProgramPointId) {
        self.successors.entry(from).or_default().push(to);
        self.predecessors.entry(to).or_default().push(from);
    }
}

impl RegionCfgView for InMemoryCfgView {
    fn successors(&self, point: ProgramPointId) -> Vec<ProgramPointId> {
        self.successors.get(&point).cloned().unwrap_or_default()
    }

    fn predecessors(&self, point: ProgramPointId) -> Vec<ProgramPointId> {
        self.predecessors.get(&point).cloned().unwrap_or_default()
    }
}
