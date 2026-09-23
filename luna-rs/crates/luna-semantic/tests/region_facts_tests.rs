//! Unit Tests for REGION-01D-A: CFG Point & Boundary Fact Model.
//!
//! Verifies:
//! - Dual-fact separation: Point Facts vs. Boundary Facts (FACT-01)
//! - One CFG edge terminating multiple dynamic region instances (FACT-02)
//! - Concrete point domain bound vs. symbolic static bound distinction (FACT-03)
//! - Contract & External region non-realizability (FACT-04)
//! - Canonical ProgramPoint and CfgEdge identity & indexing (FACT-05)
//! - Boundary exit deduplication (FACT-06)
//! - Multiple use requirements per region (FACT-07)

use std::collections::HashSet;
use luna_semantic::region::{
    is_point_realizable, CfgEdge, CfgEdgeId, PointDomainBound, ProgramPoint, ProgramPointId,
    RealizationFacts, RegionBoundaryExit, RegionGraph, RegionKind, RegionUseRequirement,
};

#[test]
fn facts_01_dual_fact_separation() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();
    let r_loop = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();

    let mut facts = RealizationFacts::new();

    // 1. Point fact: r_ref is used at program point 42
    let p_use = ProgramPointId::from_raw(42);
    facts.add_use_requirement(r_ref, p_use, None);

    // 2. Boundary fact: edge 5 terminates dynamic instance of r_loop
    let e_backedge = CfgEdgeId::from_raw(5);
    facts.add_boundary_exit(e_backedge, r_loop);

    assert_eq!(facts.requirements().len(), 1);
    assert_eq!(
        facts.requirements()[0],
        RegionUseRequirement::new(r_ref, p_use, None)
    );

    assert_eq!(facts.boundary_exits().len(), 1);
    assert_eq!(
        facts.boundary_exits()[0],
        RegionBoundaryExit::new(e_backedge, r_loop)
    );
}

#[test]
fn facts_02_one_edge_terminates_multiple_nested_regions() {
    let mut graph = RegionGraph::new();
    let r_inner = graph.add_region(RegionKind::Iteration { loop_id: 2 }).unwrap();
    let r_outer = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();
    let r_scope = graph.add_region(RegionKind::Lexical { scope: 10 }).unwrap();

    let mut facts = RealizationFacts::new();

    // Return or break edge from nested loop terminates inner loop, outer loop, and lexical scope
    let e_break_all = CfgEdgeId::from_raw(99);
    facts.add_boundary_exit(e_break_all, r_inner);
    facts.add_boundary_exit(e_break_all, r_outer);
    facts.add_boundary_exit(e_break_all, r_scope);

    assert_eq!(facts.boundary_exits().len(), 3);

    let terminated = facts.terminated_regions_at_edge(e_break_all).unwrap();
    assert_eq!(terminated.len(), 3);
    assert!(terminated.contains(&r_inner));
    assert!(terminated.contains(&r_outer));
    assert!(terminated.contains(&r_scope));
}

#[test]
fn facts_03_point_domain_bound_distinction() {
    let mut graph = RegionGraph::new();
    let r_local = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();

    let mut allowed = HashSet::new();
    allowed.insert(ProgramPointId::from_raw(10));
    allowed.insert(ProgramPointId::from_raw(11));
    allowed.insert(ProgramPointId::from_raw(12));

    let mut facts = RealizationFacts::new();
    facts.set_allowed_domain(r_local, allowed.clone());

    let domain = facts.allowed_domain(r_local).unwrap();
    assert_eq!(domain, &allowed);
    assert!(domain.contains(&ProgramPointId::from_raw(10)));
    assert!(!domain.contains(&ProgramPointId::from_raw(99)));

    let domain_bound = PointDomainBound::new(r_local, allowed);
    assert_eq!(domain_bound.region, r_local);
    assert_eq!(domain_bound.allowed_points.len(), 3);
}

#[test]
fn facts_04_contract_region_non_realizability() {
    // Concrete / local regions are point-realizable
    assert!(is_point_realizable(&RegionKind::Program));
    assert!(is_point_realizable(&RegionKind::Lexical { scope: 1 }));
    assert!(is_point_realizable(&RegionKind::Iteration { loop_id: 1 }));
    assert!(is_point_realizable(&RegionKind::Inference { variable: 0 }));

    // Abstract boundary regions are NOT point-realizable in local CFG without instantiation
    assert!(!is_point_realizable(&RegionKind::Contract));
    assert!(!is_point_realizable(&RegionKind::External { boundary: 0 }));
}

#[test]
fn facts_05_program_point_and_cfg_edge_identity() {
    let p1 = ProgramPoint::new(0, 1);
    let p2 = ProgramPoint::new(0, 1);
    let p3 = ProgramPoint::new(1, 0);

    assert_eq!(p1, p2);
    assert_ne!(p1, p3);

    let pid1 = ProgramPointId::from_raw(100);
    assert_eq!(pid1.index(), 100);

    let e1 = CfgEdge::new(0, 1);
    let e2 = CfgEdge::new(0, 1);
    let e3 = CfgEdge::new(1, 2);

    assert_eq!(e1, e2);
    assert_ne!(e1, e3);

    let eid1 = CfgEdgeId::from_raw(200);
    assert_eq!(eid1.index(), 200);
}

#[test]
fn facts_06_boundary_exit_deduplication() {
    let mut graph = RegionGraph::new();
    let r_loop = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();

    let mut facts = RealizationFacts::new();
    let edge = CfgEdgeId::from_raw(7);

    facts.add_boundary_exit(edge, r_loop);
    facts.add_boundary_exit(edge, r_loop); // Duplicate insertion

    assert_eq!(facts.boundary_exits().len(), 1);
    assert_eq!(facts.terminated_regions_at_edge(edge).unwrap().len(), 1);
}

#[test]
fn facts_07_multiple_use_requirements_per_region() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let mut facts = RealizationFacts::new();
    let p1 = ProgramPointId::from_raw(10);
    let p2 = ProgramPointId::from_raw(20);

    facts.add_use_requirement(r_ref, p1, None);
    facts.add_use_requirement(r_ref, p2, None);

    assert_eq!(facts.requirements().len(), 2);
    let points = facts.direct_required_points(r_ref).unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0], p1);
    assert_eq!(points[1], p2);
}
