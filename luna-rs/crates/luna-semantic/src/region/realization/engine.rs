//! Point Requirement Propagation & Realization Engine (REGION-01D-B).
//!
//! # Architectural Role & Invariants
//! This engine propagates required program validity points upward through Outlives relations,
//! validates fixed concrete point domains (lexical scopes), and checks dynamic boundary
//! crossing legality without inspecting or modifying loan/capability exclusivity.
//!
//! ### Core Design Invariants:
//! 1. **Flow Domain, Not Scalar Endpoint**:
//!    `last_required_point(region)` does not exist on general CFGs (branches, loops).
//!    The engine exposes [`required_points`], [`is_required_at`], and [`maximal_required_points`]
//!    (the required-use frontier).
//! 2. **Minimum Required Validity Boundary**:
//!    01D-B computes minimum required validity points and propagates them upward along
//!    $\succeq$ ($R_x \succeq R_r \land p \in \text{Required}(R_r) \implies p \in \text{Required}(R_x)$).
//!    It is not a full loan liveness solver; carrier liveness is integrated in 01D-C.
//! 3. **Dynamic Boundary Rule**:
//!    Crossing a CFG edge $e$ is invalid iff the reference's required validity depends on
//!    a dynamic region instance terminated by that edge. References created inside loops
//!    borrowing outer data are permitted to cross loop exits.
//! 4. **Decoupled Value Provenance**:
//!    The region layer answers `region_valid_at(R, p)` and `region_may_cross(R, e)`.
//!    Higher-level provenance maps references to regions.
//! 5. **Contract Region Non-Realizability**:
//!    Uninstantiated abstract regions (`Contract`, `External`) cannot be assigned local
//!    CFG points. Attempting local realization on uninstantiated contract regions is
//!    rejected as a malformed integration state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use luna_common::Span;

use crate::region::id::RegionId;
use crate::region::kind::RegionKind;
use crate::region::solver::{RegionSolution, SccId};

use super::cfg::RegionCfgView;
use super::facts::{is_point_realizable, CfgEdgeId, ProgramPointId, RealizationFacts};

/// Errors encountered during region realization, domain validation, or boundary crossing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationError {
    /// An uninstantiated abstract region (Contract or External) was requested
    /// for concrete CFG point realization without callsite instantiation.
    UninstantiatedAbstractRegion {
        region: RegionId,
        kind: RegionKind,
        point: Option<ProgramPointId>,
        span: Option<Span>,
    },

    /// A region was required at a program point outside its allowed domain (E3005).
    PointDomainViolation {
        region: RegionId,
        point: ProgramPointId,
        span: Option<Span>,
    },

    /// A region attempted to cross a CFG edge that terminates a dynamic instance it depends on.
    BoundaryViolation {
        region: RegionId,
        edge: CfgEdgeId,
        terminated_region: RegionId,
        span: Option<Span>,
    },
}

impl fmt::Display for RealizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UninstantiatedAbstractRegion { region, kind, point, .. } => {
                if let Some(p) = point {
                    write!(
                        f,
                        "Uninstantiated abstract region {:?} ({:?}) cannot be realized at local CFG point {}",
                        region, kind, p.index()
                    )
                } else {
                    write!(
                        f,
                        "Uninstantiated abstract region {:?} ({:?}) cannot participate in local CFG point realization",
                        region, kind
                    )
                }
            }
            Self::PointDomainViolation { region, point, .. } => {
                write!(
                    f,
                    "Region {:?} is required at program point {} outside its allowed domain",
                    region,
                    point.index()
                )
            }
            Self::BoundaryViolation { region, edge, terminated_region, .. } => {
                write!(
                    f,
                    "Region {:?} cannot cross CFG edge {} because it depends on terminated dynamic instance {:?}",
                    region,
                    edge.index(),
                    terminated_region
                )
            }
        }
    }
}

impl std::error::Error for RealizationError {}

/// Derived realization state containing upward-propagated point sets and boundary verifiers.
#[derive(Debug)]
pub struct RegionRealization<'a> {
    solution: &'a RegionSolution<'a>,
    facts: &'a RealizationFacts,
    /// Maps each RegionId to its set of required ProgramPointIds (after upward propagation).
    required_points: HashMap<RegionId, HashSet<ProgramPointId>>,
}

impl<'a> RegionRealization<'a> {
    /// Returns the symbolic region solution.
    #[inline]
    pub fn solution(&self) -> &'a RegionSolution<'a> {
        self.solution
    }

    /// Returns the realization facts input.
    #[inline]
    pub fn facts(&self) -> &'a RealizationFacts {
        self.facts
    }

    /// Returns all program points where `region`'s validity is required, if any.
    #[inline]
    pub fn required_points(&self, region: RegionId) -> Option<&HashSet<ProgramPointId>> {
        self.required_points.get(&region)
    }

    /// Evaluates whether region `region`'s validity is required at program point `point`.
    #[inline]
    pub fn is_required_at(&self, region: RegionId, point: ProgramPointId) -> bool {
        self.required_points
            .get(&region)
            .map(|pts| pts.contains(&point))
            .unwrap_or(false)
    }

    /// Computes the maximal required points (required-use frontier) for a region.
    ///
    /// # CFG Invariant
    /// On branching CFGs, multiple maximal points can coexist with no total ordering.
    /// In loops with cyclic uses, all mutually reachable loop uses remain in the frontier
    /// unless strictly dominated by a downstream exit point.
    pub fn maximal_required_points<C: RegionCfgView>(
        &self,
        region: RegionId,
        cfg: &C,
    ) -> Vec<ProgramPointId> {
        let Some(req_set) = self.required_points.get(&region) else {
            return Vec::new();
        };

        if req_set.is_empty() {
            return Vec::new();
        }
        if req_set.len() == 1 {
            return req_set.iter().copied().collect();
        }

        let mut maximal = Vec::new();
        for &p in req_set {
            // A point p is strictly dominated if there exists another required point q
            // reachable from p such that p is NOT reachable from q.
            let mut strictly_dominated = false;
            for &q in req_set {
                if p == q {
                    continue;
                }
                if can_reach(p, q, cfg) && !can_reach(q, p, cfg) {
                    strictly_dominated = true;
                    break;
                }
            }
            if !strictly_dominated {
                maximal.push(p);
            }
        }

        maximal.sort_by_key(|p| p.index());
        maximal
    }

    /// Evaluates whether region `region` is valid at program point `point`.
    ///
    /// # Semantics
    /// A region is valid at point `p` iff:
    /// 1. It is point-realizable (not an uninstantiated `Contract` or `External` variable).
    /// 2. For every provider region $S$ that outlives `region` ($S \succeq region$), if $S$
    ///    has a defined allowed domain $D(S)$, then $p \in D(S)$.
    pub fn region_valid_at(
        &self,
        region: RegionId,
        point: ProgramPointId,
    ) -> Result<bool, RealizationError> {
        if let Some(node) = self.solution.graph().get_node(region) {
            if !is_point_realizable(&node.kind) {
                return Err(RealizationError::UninstantiatedAbstractRegion {
                    region,
                    kind: node.kind,
                    point: Some(point),
                    span: None,
                });
            }
        }

        // Every provider S providing validity to `region` must be alive at `point`
        for i in 0..self.solution.graph().nodes().len() {
            let provider = RegionId::from_raw(i as u32);
            if self.solution.outlives(provider, region) {
                if let Some(allowed) = self.facts.allowed_domain(provider) {
                    if !allowed.contains(&point) {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    /// Evaluates whether region `region` is legally allowed to cross CFG edge `edge`.
    ///
    /// # Dynamic Boundary Rule
    /// Crossing edge `edge` is invalid iff `region`'s required validity depends on
    /// any dynamic region instance terminated by `edge` ($R_{term} \succeq region$).
    #[inline]
    pub fn region_may_cross(&self, region: RegionId, edge: CfgEdgeId) -> bool {
        if let Some(terminated) = self.facts.terminated_regions_at_edge(edge) {
            for &term in terminated {
                if self.solution.outlives(term, region) {
                    return false;
                }
            }
        }
        true
    }

    /// Asserts that region `region` may legally cross CFG edge `edge`.
    ///
    /// Returns `Ok(())` if crossing is legal, or `Err(RealizationError::BoundaryViolation)`
    /// identifying the terminated dynamic instance.
    pub fn check_crossing(
        &self,
        region: RegionId,
        edge: CfgEdgeId,
        span: Option<Span>,
    ) -> Result<(), RealizationError> {
        if let Some(terminated) = self.facts.terminated_regions_at_edge(edge) {
            for &term in terminated {
                if self.solution.outlives(term, region) {
                    return Err(RealizationError::BoundaryViolation {
                        region,
                        edge,
                        terminated_region: term,
                        span,
                    });
                }
            }
        }
        Ok(())
    }

    /// Validates all fixed point domain bounds against the computed required points.
    ///
    /// Returns `Ok(())` if all required points fall within their allowed domains,
    /// or `Err(RealizationError::PointDomainViolation)` on the first violation.
    pub fn validate_point_domains(&self) -> Result<(), RealizationError> {
        for (&region, req_pts) in &self.required_points {
            if let Some(allowed) = self.facts.allowed_domain(region) {
                for &p in req_pts {
                    if !allowed.contains(&p) {
                        let span = self
                            .facts
                            .requirements()
                            .iter()
                            .find(|r| r.region == region && r.point == p)
                            .and_then(|r| r.span);
                        return Err(RealizationError::PointDomainViolation {
                            region,
                            point: p,
                            span,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Helper BFS to check if `target` is reachable from `start` via a non-empty forward CFG path.
fn can_reach(start: ProgramPointId, target: ProgramPointId, cfg: &impl RegionCfgView) -> bool {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    for succ in cfg.successors(start) {
        if succ == target {
            return true;
        }
        if visited.insert(succ) {
            queue.push_back(succ);
        }
    }

    while let Some(curr) = queue.pop_front() {
        for succ in cfg.successors(curr) {
            if succ == target {
                return true;
            }
            if visited.insert(succ) {
                queue.push_back(succ);
            }
        }
    }

    false
}

/// Realizes concrete CFG point requirements from symbolic solution and realization facts.
///
/// ### Algorithmic Steps:
/// 1. Verifies that no uninstantiated abstract regions (`Contract`, `External`) have
///    direct local point requirements.
/// 2. Initializes requirement point-sets per SCC.
/// 3. Propagates requirements upward along the Outlives relation ($U \succeq V \implies \text{Req}(V) \subseteq \text{Req}(U)$).
/// 4. Rejects any uninstantiated abstract regions that receive propagated concrete CFG points.
/// 5. Validates concrete point domain bounds.
pub fn realize_regions<'a>(
    solution: &'a RegionSolution<'a>,
    facts: &'a RealizationFacts,
) -> Result<RegionRealization<'a>, RealizationError> {
    // 1. Verify direct requirements on point-realizable regions
    for req in facts.requirements() {
        if let Some(node) = solution.graph().get_node(req.region) {
            if !is_point_realizable(&node.kind) {
                return Err(RealizationError::UninstantiatedAbstractRegion {
                    region: req.region,
                    kind: node.kind,
                    point: Some(req.point),
                    span: req.span,
                });
            }
        }
    }

    let num_sccs = solution.classes().len();
    let mut scc_points: Vec<HashSet<ProgramPointId>> = vec![HashSet::new(); num_sccs];

    // 2. Populate direct requirements per SCC
    for req in facts.requirements() {
        if let Some(scc) = solution.scc_of(req.region) {
            scc_points[scc.index() as usize].insert(req.point);
        }
    }

    // 3. Upward propagation: if SCC u outlives SCC v (u ⪰ v), points from v flow into u
    let mut propagated_scc_points = scc_points.clone();
    for u in 0..num_sccs {
        let u_scc = SccId(u as u32);
        for v in 0..num_sccs {
            if u != v {
                let v_scc = SccId(v as u32);
                if solution.scc_outlives(u_scc, v_scc) {
                    propagated_scc_points[u].extend(scc_points[v].iter().copied());
                }
            }
        }
    }

    // 4. Map propagated points back to regions and verify abstract region non-realizability
    let mut required_map: HashMap<RegionId, HashSet<ProgramPointId>> = HashMap::new();

    for u in 0..num_sccs {
        let u_scc = SccId(u as u32);
        let pts = &propagated_scc_points[u];
        if pts.is_empty() {
            continue;
        }

        if let Some(members) = solution.scc_members(u_scc) {
            for &member in members {
                if let Some(node) = solution.graph().get_node(member) {
                    if !is_point_realizable(&node.kind) {
                        return Err(RealizationError::UninstantiatedAbstractRegion {
                            region: member,
                            kind: node.kind,
                            point: pts.iter().next().copied(),
                            span: None,
                        });
                    }
                }
                required_map.insert(member, pts.clone());
            }
        }
    }

    let realization = RegionRealization {
        solution,
        facts,
        required_points: required_map,
    };

    // 5. Validate fixed point domains
    realization.validate_point_domains()?;

    Ok(realization)
}
