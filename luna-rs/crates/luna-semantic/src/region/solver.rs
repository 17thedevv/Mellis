//! Symbolic Region Relation Solver (REGION-01C).
//!
//! # Architectural Role & Boundary
//! The `Symbolic Region Relation Solver` proves validity-domain ordering and equivalence
//! relations directly from the canonical `RegionGraph` and axiomatic domain rules.
//!
//! ### Core Guarantees & Boundary Invariants:
//! 1. **Non-Mutating Execution**:
//!    `RegionGraph` is the immutable source of semantic truth. The solver produces
//!    a derived query object, [`RegionSolution`], without injecting transitive edges
//!    into `RegionGraph.constraints`.
//! 2. **Effective Relation with Program Axiom**:
//!    By language axiom, $\text{Program} \succeq R \quad \forall R$.
//!    The working relation graph treats $\text{Program} \to R$ as valid reachability edges.
//!    If an arbitrary region outlives Program ($A \succeq \text{Program}$), it collapses
//!    into the same SCC ($A \equiv \text{Program}$).
//! 3. **Tarjan SCC & Deterministic Condensation**:
//!    Cycles ($A \succeq B \land B \succeq A$) normalize into equivalence classes.
//!    SCCs and condensation edges are deterministically sorted by representative `RegionId`
//!    to ensure strictly reproducible indexing and queries.
//! 4. **Pure Positive Graphs Are Always Solvable**:
//!    Positive outlives constraints alone never produce contradictions. Contradiction
//!    is strictly a function of supplied negative bounds ([`StaticBound`]).
//! 5. **Proof Paths Retain 3-Axis Metadata**:
//!    The explanation engine produces concrete [`ProofPath`] steps referencing the original
//!    `ConstraintId` and 3-axis metadata without duplication.

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use luna_common::Span;

use super::id::{ConstraintId, RegionId};
use super::graph::RegionGraph;

/// An opaque identifier representing a strongly connected component / equivalence class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SccId(pub u32);

impl SccId {
    /// Returns the 0-based raw index of this SCC.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// An equivalence class of regions proven to be mutually outliving ($A \equiv B$).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceClass {
    /// Canonical identifier for this class.
    pub id: SccId,
    /// Canonical representative region (the member with the smallest `RegionId`).
    pub representative: RegionId,
    /// All region members belonging to this equivalence class, sorted by `RegionId`.
    pub members: Vec<RegionId>,
}

/// A single step in a proof explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofStep {
    /// A direct outlives constraint recorded in the `RegionGraph`.
    Direct {
        from: RegionId,
        to: RegionId,
        constraint_id: ConstraintId,
    },
    /// Derived from the fundamental axiom: $\text{Program} \succeq R \quad \forall R$.
    ProgramAxiom {
        to: RegionId,
    },
    /// Derived from reflexivity: $R \succeq R \quad \forall R$.
    Reflexive {
        region: RegionId,
    },
}

/// An ordered sequence of proof steps demonstrating why $A \succeq B$.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPath {
    pub steps: Vec<ProofStep>,
}

/// A static / negative bound on region relations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticBound {
    pub kind: StaticBoundKind,
    pub span: Option<Span>,
}

impl StaticBound {
    /// Constructs a `CannotOutlive` bound asserting $\neg(sup \succeq sub)$.
    pub fn cannot_outlive(sup: RegionId, sub: RegionId, span: Option<Span>) -> Self {
        Self {
            kind: StaticBoundKind::CannotOutlive { sup, sub },
            span,
        }
    }
}

/// The kind of static bound to validate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticBoundKind {
    /// Asserts that `sup` cannot outlive `sub` ($\neg(sup \succeq sub)$).
    /// If the constraint graph proves $sup \succeq sub$, a contradiction occurs.
    CannotOutlive {
        sup: RegionId,
        sub: RegionId,
    },
}

/// Represents a contradiction when a static bound is violated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundViolation {
    pub bound: StaticBound,
    pub proof: ProofPath,
}

/// Derived solver result containing normalized equivalence classes, condensation DAG,
/// and transitive relation entailment.
#[derive(Debug)]
pub struct RegionSolution<'a> {
    graph: &'a RegionGraph,
    /// Maps each `RegionId` (by raw index) to its canonical `SccId`.
    region_to_scc: Vec<SccId>,
    /// All equivalence classes, indexed by `SccId`.
    classes: Vec<EquivalenceClass>,
    /// Outgoing edges in the condensation DAG, sorted by destination `SccId`.
    condensation_adj: Vec<Vec<SccId>>,
    /// Precomputed transitive reachability matrix on condensation DAG.
    /// `reachability[u][v] == true` iff SCC `u` outlives SCC `v`.
    reachability: Vec<Vec<bool>>,
}

impl<'a> RegionSolution<'a> {
    /// Returns the underlying `RegionGraph` referenced by this solution.
    #[inline]
    pub fn graph(&self) -> &'a RegionGraph {
        self.graph
    }

    /// Evaluates whether region `sup` symbolically outlives region `sub` ($sup \succeq sub$).
    ///
    /// # Semantics
    /// - Reflexive: $R \succeq R$ is always true for any registered region.
    /// - Program Axiom: $\text{Program} \succeq R$ is always true for any registered region.
    /// - Equivalence: If $sup \equiv sub$, outlives holds in both directions.
    /// - Transitive: Holds if `sub`'s SCC is reachable from `sup`'s SCC in the condensation DAG.
    pub fn outlives(&self, sup: RegionId, sub: RegionId) -> bool {
        let Some(sup_scc) = self.scc_of(sup) else {
            return false;
        };
        let Some(sub_scc) = self.scc_of(sub) else {
            return false;
        };
        self.scc_outlives(sup_scc, sub_scc)
    }

    /// Evaluates whether SCC `sup` outlives SCC `sub` in the condensation DAG.
    #[inline]
    pub fn scc_outlives(&self, sup: SccId, sub: SccId) -> bool {
        let sup_idx = sup.index() as usize;
        let sub_idx = sub.index() as usize;
        if sup_idx < self.reachability.len() && sub_idx < self.reachability.len() {
            self.reachability[sup_idx][sub_idx]
        } else {
            false
        }
    }

    /// Evaluates whether two regions are mutually outliving and belong to the same
    /// equivalence class ($a \equiv b$).
    pub fn equivalent(&self, a: RegionId, b: RegionId) -> bool {
        let Some(a_scc) = self.scc_of(a) else {
            return false;
        };
        let Some(b_scc) = self.scc_of(b) else {
            return false;
        };
        a_scc == b_scc
    }

    /// Returns the canonical `SccId` for a given `RegionId`, if registered in the graph.
    pub fn scc_of(&self, r: RegionId) -> Option<SccId> {
        self.region_to_scc.get(r.index() as usize).copied()
    }

    /// Returns the slice of `RegionId` members belonging to the specified SCC.
    pub fn scc_members(&self, scc: SccId) -> Option<&[RegionId]> {
        self.classes
            .get(scc.index() as usize)
            .map(|c| c.members.as_slice())
    }

    /// Returns the canonical representative `RegionId` for an SCC.
    pub fn representative(&self, scc: SccId) -> Option<RegionId> {
        self.classes
            .get(scc.index() as usize)
            .map(|c| c.representative)
    }

    /// Returns all canonical equivalence classes.
    pub fn classes(&self) -> &[EquivalenceClass] {
        &self.classes
    }

    /// Returns outgoing condensation DAG edges from the given SCC.
    pub fn condensation_edges(&self, scc: SccId) -> Option<&[SccId]> {
        self.condensation_adj
            .get(scc.index() as usize)
            .map(|v| v.as_slice())
    }

    /// Constructs a step-by-step [`ProofPath`] explaining why `sup` outlives `sub`.
    ///
    /// Returns `None` if the graph does not prove that `sup` outlives `sub`.
    /// For verified relations, searches the effective graph using deterministic BFS,
    /// retaining direct constraint origins and axiomatic Program steps.
    pub fn explain_outlives(&self, sup: RegionId, sub: RegionId) -> Option<ProofPath> {
        if !self.outlives(sup, sub) {
            return None;
        }

        // Case 1: Reflexive identity
        if sup == sub {
            return Some(ProofPath {
                steps: vec![ProofStep::Reflexive { region: sup }],
            });
        }

        // Case 2: Deterministic shortest-path search on effective relation graph.
        // Effective edges:
        // - Direct constraints from graph
        // - Program -> R virtual axiomatic edge (if current == Program)
        let program_id = self.graph.program_region();
        let num_nodes = self.graph.nodes().len();

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut came_from: HashMap<RegionId, (RegionId, ProofStep)> = HashMap::new();

        visited.insert(sup);
        queue.push_back(sup);

        while let Some(curr) = queue.pop_front() {
            if curr == sub {
                break;
            }

            // Collect outgoing candidate transitions deterministically.
            // Map target -> min ConstraintId for direct edges.
            let mut direct_candidates: HashMap<RegionId, ConstraintId> = HashMap::new();
            for c in self.graph.constraints() {
                match c.kind {
                    super::constraint::ConstraintKind::Outlives {
                        sup: c_sup,
                        sub: c_sub,
                    } => {
                        if c_sup == curr {
                            direct_candidates
                                .entry(c_sub)
                                .and_modify(|existing| {
                                    if c.id.index() < existing.index() {
                                        *existing = c.id;
                                    }
                                })
                                .or_insert(c.id);
                        }
                    }
                }
            }

            // Build deterministic list of candidates sorted by target RegionId
            let mut candidates: Vec<(RegionId, ProofStep)> = Vec::new();

            for (&target, &cid) in &direct_candidates {
                candidates.push((
                    target,
                    ProofStep::Direct {
                        from: curr,
                        to: target,
                        constraint_id: cid,
                    },
                ));
            }

            // If curr is Program, also offer ProgramAxiom to all non-direct target regions
            if curr == program_id {
                for i in 0..num_nodes {
                    let target = RegionId::from_raw(i as u32);
                    if target != curr && !direct_candidates.contains_key(&target) {
                        candidates.push((target, ProofStep::ProgramAxiom { to: target }));
                    }
                }
            }

            // Sort candidates by target RegionId for total determinism
            candidates.sort_by_key(|(target, _)| target.index());

            for (target, step) in candidates {
                if visited.insert(target) {
                    came_from.insert(target, (curr, step));
                    queue.push_back(target);
                }
            }
        }

        // Reconstruct path from sub back to sup
        if !came_from.contains_key(&sub) {
            return None;
        }

        let mut steps = Vec::new();
        let mut curr = sub;
        while curr != sup {
            let (prev, step) = came_from.remove(&curr)?;
            steps.push(step);
            curr = prev;
        }
        steps.reverse();

        Some(ProofPath { steps })
    }

    /// Validates a set of static bounds against this solution.
    ///
    /// Returns `Ok(())` if all bounds are respected, or `Err(BoundViolation)`
    /// with an explanation proof path on the first contradiction.
    pub fn validate_static_bounds(&self, bounds: &[StaticBound]) -> Result<(), BoundViolation> {
        for bound in bounds {
            match bound.kind {
                StaticBoundKind::CannotOutlive { sup, sub } => {
                    if self.outlives(sup, sub) {
                        let proof = self
                            .explain_outlives(sup, sub)
                            .unwrap_or_else(|| ProofPath { steps: Vec::new() });
                        return Err(BoundViolation {
                            bound: bound.clone(),
                            proof,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Solves symbolic lifetime relations for a `RegionGraph`.
///
/// Builds the effective working relation (direct edges + axiomatic Program edges),
/// normalizes cycles into SCC equivalence classes via Tarjan's algorithm,
/// canonically re-indexes components, and computes transitive reachability.
///
/// Pure positive graphs never fail; negative bound checks are performed separately via
/// [`RegionSolution::validate_static_bounds`].
pub fn solve_region_graph(graph: &RegionGraph) -> RegionSolution<'_> {
    let num_nodes = graph.nodes().len();
    let program_id = graph.program_region();

    // 1. Run Tarjan's SCC algorithm on the effective working relation graph.
    let mut tarjan = TarjanContext::new(num_nodes, graph, program_id);
    for u in 0..num_nodes {
        if tarjan.indices[u].is_none() {
            tarjan.strongconnect(u);
        }
    }

    // 2. Deterministic canonicalization of SCCs:
    // - Sort members of each SCC by RegionId
    // - Identify representative as members[0]
    // - Sort all SCCs by representative RegionId
    let mut raw_sccs = tarjan.sccs;
    for scc in &mut raw_sccs {
        scc.sort_by_key(|r| r.index());
    }
    raw_sccs.sort_by_key(|scc| scc[0].index());

    // 3. Assign canonical SccId(0..N) and construct region_to_scc mapping
    let num_sccs = raw_sccs.len();
    let mut region_to_scc = vec![SccId(0); num_nodes];
    let mut classes = Vec::with_capacity(num_sccs);

    for (scc_idx, members) in raw_sccs.into_iter().enumerate() {
        let scc_id = SccId(scc_idx as u32);
        let representative = members[0];
        for &r in &members {
            region_to_scc[r.index() as usize] = scc_id;
        }
        classes.push(EquivalenceClass {
            id: scc_id,
            representative,
            members,
        });
    }

    // 4. Construct Condensation DAG adjacency
    let mut condensation_adj = vec![Vec::new(); num_sccs];
    for u_scc in 0..num_sccs {
        let mut target_sccs = HashSet::new();
        let u_scc_id = SccId(u_scc as u32);

        for &member in &classes[u_scc].members {
            // If member is Program, it has virtual axiomatic edges to all other regions
            if member == program_id {
                for other_scc in 0..num_sccs {
                    if other_scc != u_scc {
                        target_sccs.insert(SccId(other_scc as u32));
                    }
                }
            }

            // Direct constraints
            if let Some(targets) = graph.direct_outlives_targets(member) {
                for &target in targets {
                    let target_scc = region_to_scc[target.index() as usize];
                    if target_scc != u_scc_id {
                        target_sccs.insert(target_scc);
                    }
                }
            }
        }

        let mut sorted_targets: Vec<SccId> = target_sccs.into_iter().collect();
        sorted_targets.sort_by_key(|s| s.index());
        condensation_adj[u_scc] = sorted_targets;
    }

    // 5. Precompute transitive reachability matrix on condensation DAG
    let mut reachability = vec![vec![false; num_sccs]; num_sccs];
    for src in 0..num_sccs {
        reachability[src][src] = true;
        let mut queue = VecDeque::new();
        queue.push_back(SccId(src as u32));

        while let Some(curr) = queue.pop_front() {
            for &next in &condensation_adj[curr.index() as usize] {
                let next_idx = next.index() as usize;
                if !reachability[src][next_idx] {
                    reachability[src][next_idx] = true;
                    queue.push_back(next);
                }
            }
        }
    }

    RegionSolution {
        graph,
        region_to_scc,
        classes,
        condensation_adj,
        reachability,
    }
}

/// Internal helper for running Tarjan's SCC algorithm.
struct TarjanContext<'g> {
    graph: &'g RegionGraph,
    program_id: RegionId,
    num_nodes: usize,
    index_counter: u32,
    indices: Vec<Option<u32>>,
    lowlinks: Vec<u32>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    sccs: Vec<Vec<RegionId>>,
}

impl<'g> TarjanContext<'g> {
    fn new(num_nodes: usize, graph: &'g RegionGraph, program_id: RegionId) -> Self {
        Self {
            graph,
            program_id,
            num_nodes,
            index_counter: 0,
            indices: vec![None; num_nodes],
            lowlinks: vec![0; num_nodes],
            on_stack: vec![false; num_nodes],
            stack: Vec::new(),
            sccs: Vec::new(),
        }
    }

    fn strongconnect(&mut self, u: usize) {
        let idx = self.index_counter;
        self.index_counter += 1;
        self.indices[u] = Some(idx);
        self.lowlinks[u] = idx;
        self.stack.push(u);
        self.on_stack[u] = true;

        // Collect outgoing neighbors on effective working graph.
        // Neighbors are sorted by raw node index for deterministic traversal.
        let u_region = RegionId::from_raw(u as u32);
        let mut neighbors = Vec::new();

        // 1. Direct edges from graph
        if let Some(targets) = self.graph.direct_outlives_targets(u_region) {
            for &t in targets {
                neighbors.push(t.index() as usize);
            }
        }

        // 2. Axiomatic Program edges: Program -> every region
        if u_region == self.program_id {
            for v in 0..self.num_nodes {
                if v != u && !neighbors.contains(&v) {
                    neighbors.push(v);
                }
            }
        }

        neighbors.sort_unstable();

        for w in neighbors {
            match self.indices[w] {
                None => {
                    self.strongconnect(w);
                    self.lowlinks[u] = self.lowlinks[u].min(self.lowlinks[w]);
                }
                Some(w_idx) => {
                    if self.on_stack[w] {
                        self.lowlinks[u] = self.lowlinks[u].min(w_idx);
                    }
                }
            }
        }

        // If u is a root of an SCC, pop the stack
        if self.lowlinks[u] == idx {
            let mut scc_members = Vec::new();
            while let Some(w) = self.stack.pop() {
                self.on_stack[w] = false;
                scc_members.push(RegionId::from_raw(w as u32));
                if w == u {
                    break;
                }
            }
            self.sccs.push(scc_members);
        }
    }
}
