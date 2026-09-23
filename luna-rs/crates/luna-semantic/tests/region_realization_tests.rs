//! Unit Tests for REGION-01D-B: Point Requirement Propagation & Realization Engine.
//!
//! Verifies:
//! - REAL-01: Direct use requirement realization
//! - REAL-02: Upward propagation along Outlives relation
//! - REAL-03: Equivalence class propagation
//! - REAL-04: Fixed point domain bound success
//! - REAL-05: Fixed point domain bound violation (E3005)
//! - REAL-06: `region_valid_at` provider check
//! - POINT-01: Branch with two maximal uses (no unique last point)
//! - POINT-02: Linear code unique maximal point
//! - POINT-03: Loop cyclic frontier
//! - POINT-04: Loop with downstream exit
//! - BOUNDARY-01: Reference created inside iteration borrowing outer data allowed to cross break
//! - BOUNDARY-02: Reference borrowing iteration-local data cannot cross iteration exit
//! - BOUNDARY-03: One CFG edge terminates multiple nested dynamic regions
//! - CONTRACT-01: Uninstantiated Contract region rejected for local CFG point realization
//! - CONTRACT-02: Uninstantiated External region rejected for local CFG point realization

use std::collections::HashSet;
use luna_semantic::region::{
    realize_regions, solve_region_graph, BodyConstraintGenerator, CfgEdgeId, InMemoryCfgView,
    ProgramPointId, RealizationError, RealizationFacts, RegionGraph, RegionKind,
};

#[test]
fn real_01_direct_use_requirement() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let mut facts = RealizationFacts::new();
    let p_use = ProgramPointId::from_raw(10);
    facts.add_use_requirement(r_ref, p_use, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    assert!(realization.is_required_at(r_ref, p_use));
    assert_eq!(
        realization.required_points(r_ref).unwrap().len(),
        1
    );
    assert!(!realization.is_required_at(r_ref, ProgramPointId::from_raw(99)));
}

#[test]
fn real_02_upward_propagation_outlives() {
    let mut graph = RegionGraph::new();
    let r_referent = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    // Referent outlives reference: R_referent ⪰ R_ref
    body_gen
        .generate_borrow_constraint(&mut graph, r_referent, r_ref, None)
        .unwrap();

    let mut facts = RealizationFacts::new();
    let p_use = ProgramPointId::from_raw(42);
    facts.add_use_requirement(r_ref, p_use, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // R_ref requires p_use
    assert!(realization.is_required_at(r_ref, p_use));
    // Upward propagation: R_referent ⪰ R_ref => R_referent requires p_use
    assert!(realization.is_required_at(r_referent, p_use));
    // Axiomatic root: Program ⪰ R_referent => Program requires p_use
    assert!(realization.is_required_at(graph.program_region(), p_use));
}

#[test]
fn real_03_equivalence_class_propagation() {
    let mut graph = RegionGraph::new();
    let r_a = graph.add_region(RegionKind::Inference { variable: 1 }).unwrap();
    let r_b = graph.add_region(RegionKind::Inference { variable: 2 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    // Mutual outlives: R_a ⪰ R_b and R_b ⪰ R_a => R_a ≡ R_b
    body_gen.generate_borrow_constraint(&mut graph, r_a, r_b, None).unwrap();
    body_gen.generate_borrow_constraint(&mut graph, r_b, r_a, None).unwrap();

    let mut facts = RealizationFacts::new();
    let p1 = ProgramPointId::from_raw(10);
    let p2 = ProgramPointId::from_raw(20);
    facts.add_use_requirement(r_a, p1, None);
    facts.add_use_requirement(r_b, p2, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // Both points exist in both equivalent regions
    assert!(realization.is_required_at(r_a, p1));
    assert!(realization.is_required_at(r_a, p2));
    assert!(realization.is_required_at(r_b, p1));
    assert!(realization.is_required_at(r_b, p2));
}

#[test]
fn real_04_point_domain_bound_success() {
    let mut graph = RegionGraph::new();
    let r_scope = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    body_gen.generate_borrow_constraint(&mut graph, r_scope, r_ref, None).unwrap();

    let mut allowed = HashSet::new();
    allowed.insert(ProgramPointId::from_raw(1));
    allowed.insert(ProgramPointId::from_raw(2));
    allowed.insert(ProgramPointId::from_raw(3));

    let mut facts = RealizationFacts::new();
    facts.set_allowed_domain(r_scope, allowed);
    facts.add_use_requirement(r_ref, ProgramPointId::from_raw(2), None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts);
    assert!(realization.is_ok());
}

#[test]
fn real_05_point_domain_bound_violation() {
    let mut graph = RegionGraph::new();
    let r_scope = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    body_gen.generate_borrow_constraint(&mut graph, r_scope, r_ref, None).unwrap();

    let mut allowed = HashSet::new();
    allowed.insert(ProgramPointId::from_raw(1));
    allowed.insert(ProgramPointId::from_raw(2));

    let mut facts = RealizationFacts::new();
    facts.set_allowed_domain(r_scope, allowed);
    // Use requirement outside allowed domain
    let p_escaped = ProgramPointId::from_raw(99);
    facts.add_use_requirement(r_ref, p_escaped, None);

    let solution = solve_region_graph(&graph);
    let result = realize_regions(&solution, &facts);

    assert!(result.is_err());
    match result.unwrap_err() {
        RealizationError::PointDomainViolation { region, point, .. } => {
            assert_eq!(region, r_scope);
            assert_eq!(point, p_escaped);
        }
        other => panic!("Expected PointDomainViolation, got {:?}", other),
    }
}

#[test]
fn real_06_region_valid_at() {
    let mut graph = RegionGraph::new();
    let r_local = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let r_outer = graph.add_region(RegionKind::Lexical { scope: 0 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    body_gen.generate_borrow_constraint(&mut graph, r_local, r_ref, None).unwrap();

    let mut local_domain = HashSet::new();
    local_domain.insert(ProgramPointId::from_raw(1));
    local_domain.insert(ProgramPointId::from_raw(2));

    let mut outer_domain = HashSet::new();
    outer_domain.insert(ProgramPointId::from_raw(1));
    outer_domain.insert(ProgramPointId::from_raw(2));
    outer_domain.insert(ProgramPointId::from_raw(3));

    let mut facts = RealizationFacts::new();
    facts.set_allowed_domain(r_local, local_domain);
    facts.set_allowed_domain(r_outer, outer_domain);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // r_ref depends on r_local: valid at 1, invalid at 3
    assert_eq!(realization.region_valid_at(r_ref, ProgramPointId::from_raw(1)), Ok(true));
    assert_eq!(realization.region_valid_at(r_ref, ProgramPointId::from_raw(3)), Ok(false));

    // r_outer is valid at 3
    assert_eq!(realization.region_valid_at(r_outer, ProgramPointId::from_raw(3)), Ok(true));

    // Program region has no restrictions
    assert_eq!(
        realization.region_valid_at(graph.program_region(), ProgramPointId::from_raw(99)),
        Ok(true)
    );
}

#[test]
fn point_01_branch_has_two_maximal_uses_no_unique_last_point() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let p_branch = ProgramPointId::from_raw(1);
    let p_left = ProgramPointId::from_raw(2);
    let p_right = ProgramPointId::from_raw(3);
    let p_exit = ProgramPointId::from_raw(4);

    let mut cfg = InMemoryCfgView::new();
    cfg.add_edge(p_branch, p_left);
    cfg.add_edge(p_branch, p_right);
    cfg.add_edge(p_left, p_exit);
    cfg.add_edge(p_right, p_exit);

    let mut facts = RealizationFacts::new();
    facts.add_use_requirement(r_ref, p_left, None);
    facts.add_use_requirement(r_ref, p_right, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    let maximal = realization.maximal_required_points(r_ref, &cfg);
    // On a branch, there is NO unique last point. Both branch points are maximal!
    assert_eq!(maximal, vec![p_left, p_right]);
}

#[test]
fn point_02_linear_code_unique_maximal_point() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let p1 = ProgramPointId::from_raw(1);
    let p2 = ProgramPointId::from_raw(2);
    let p3 = ProgramPointId::from_raw(3);

    let mut cfg = InMemoryCfgView::new();
    cfg.add_edge(p1, p2);
    cfg.add_edge(p2, p3);

    let mut facts = RealizationFacts::new();
    facts.add_use_requirement(r_ref, p1, None);
    facts.add_use_requirement(r_ref, p2, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    let maximal = realization.maximal_required_points(r_ref, &cfg);
    // In strictly linear code, downstream point p2 strictly dominates p1
    assert_eq!(maximal, vec![p2]);
}

#[test]
fn point_03_loop_cyclic_frontier() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let p_head = ProgramPointId::from_raw(1);
    let p_body = ProgramPointId::from_raw(2);
    let p_exit = ProgramPointId::from_raw(3);

    let mut cfg = InMemoryCfgView::new();
    cfg.add_edge(p_head, p_body);
    cfg.add_edge(p_body, p_head); // loop back-edge
    cfg.add_edge(p_head, p_exit);

    let mut facts = RealizationFacts::new();
    facts.add_use_requirement(r_ref, p_head, None);
    facts.add_use_requirement(r_ref, p_body, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    let maximal = realization.maximal_required_points(r_ref, &cfg);
    // Mutually reachable loop uses maintain the frontier without collapse
    assert_eq!(maximal, vec![p_head, p_body]);
}

#[test]
fn point_04_loop_with_downstream_exit() {
    let mut graph = RegionGraph::new();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let p_head = ProgramPointId::from_raw(1);
    let p_body = ProgramPointId::from_raw(2);
    let p_exit = ProgramPointId::from_raw(3);

    let mut cfg = InMemoryCfgView::new();
    cfg.add_edge(p_head, p_body);
    cfg.add_edge(p_body, p_head);
    cfg.add_edge(p_head, p_exit);

    let mut facts = RealizationFacts::new();
    facts.add_use_requirement(r_ref, p_body, None);
    facts.add_use_requirement(r_ref, p_exit, None);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    let maximal = realization.maximal_required_points(r_ref, &cfg);
    // Downstream exit point p_exit strictly dominates loop body use
    assert_eq!(maximal, vec![p_exit]);
}

#[test]
fn boundary_01_borrowing_outer_data_allowed_to_cross_break() {
    let mut graph = RegionGraph::new();
    let r_outer = graph.add_region(RegionKind::Lexical { scope: 0 }).unwrap();
    let r_iter = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    // Reference created inside loop iteration borrows outer object: R_outer ⪰ R_ref
    body_gen.generate_borrow_constraint(&mut graph, r_outer, r_ref, None).unwrap();

    let e_break = CfgEdgeId::from_raw(10);
    let mut facts = RealizationFacts::new();
    // Loop exit terminates iteration region
    facts.add_boundary_exit(e_break, r_iter);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // R_ref's validity source is R_outer, which is NOT terminated by e_break.
    // Allowed to cross break!
    assert!(realization.region_may_cross(r_ref, e_break));
    assert!(realization.check_crossing(r_ref, e_break, None).is_ok());
}

#[test]
fn boundary_02_borrowing_iteration_local_data_cannot_cross() {
    let mut graph = RegionGraph::new();
    let r_iter = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();
    let r_local = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();
    let r_ref = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    // Local data inside loop is bounded by iteration: R_iter ⪰ R_local
    body_gen.generate_borrow_constraint(&mut graph, r_iter, r_local, None).unwrap();
    // Reference borrows local data: R_local ⪰ R_ref
    body_gen.generate_borrow_constraint(&mut graph, r_local, r_ref, None).unwrap();

    let e_exit = CfgEdgeId::from_raw(20);
    let mut facts = RealizationFacts::new();
    facts.add_boundary_exit(e_exit, r_iter);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // R_ref's validity depends on R_iter via R_local. R_iter is terminated by e_exit.
    // Crossing must be blocked!
    assert!(!realization.region_may_cross(r_ref, e_exit));
    let crossing_result = realization.check_crossing(r_ref, e_exit, None);
    assert!(crossing_result.is_err());
    match crossing_result.unwrap_err() {
        RealizationError::BoundaryViolation {
            region,
            edge,
            terminated_region,
            ..
        } => {
            assert_eq!(region, r_ref);
            assert_eq!(edge, e_exit);
            assert_eq!(terminated_region, r_iter);
        }
        other => panic!("Expected BoundaryViolation, got {:?}", other),
    }
}

#[test]
fn boundary_03_one_cfg_edge_terminates_multiple_nested_regions() {
    let mut graph = RegionGraph::new();
    let r_inner = graph.add_region(RegionKind::Iteration { loop_id: 2 }).unwrap();
    let r_outer = graph.add_region(RegionKind::Iteration { loop_id: 1 }).unwrap();
    let r_global = graph.add_region(RegionKind::Lexical { scope: 0 }).unwrap();

    let r_ref_inner = graph.add_region(RegionKind::Inference { variable: 1 }).unwrap();
    let r_ref_outer = graph.add_region(RegionKind::Inference { variable: 2 }).unwrap();
    let r_ref_global = graph.add_region(RegionKind::Inference { variable: 3 }).unwrap();

    let body_gen = BodyConstraintGenerator::new();
    body_gen.generate_borrow_constraint(&mut graph, r_inner, r_ref_inner, None).unwrap();
    body_gen.generate_borrow_constraint(&mut graph, r_outer, r_ref_outer, None).unwrap();
    body_gen.generate_borrow_constraint(&mut graph, r_global, r_ref_global, None).unwrap();

    let e_return = CfgEdgeId::from_raw(99);
    let mut facts = RealizationFacts::new();
    // Single return edge terminates both inner loop and outer loop
    facts.add_boundary_exit(e_return, r_inner);
    facts.add_boundary_exit(e_return, r_outer);

    let solution = solve_region_graph(&graph);
    let realization = realize_regions(&solution, &facts).unwrap();

    // Inner-dependent reference cannot cross
    assert!(!realization.region_may_cross(r_ref_inner, e_return));
    // Outer-dependent reference cannot cross
    assert!(!realization.region_may_cross(r_ref_outer, e_return));
    // Global-dependent reference CAN cross
    assert!(realization.region_may_cross(r_ref_global, e_return));
    assert!(realization.check_crossing(r_ref_global, e_return, None).is_ok());
}

#[test]
fn contract_01_uninstantiated_contract_region_rejected() {
    let mut graph = RegionGraph::new();
    let r_contract = graph.add_region(RegionKind::Contract).unwrap();

    let mut facts = RealizationFacts::new();
    // Attempting to assign a local CFG point directly to an uninstantiated contract region
    facts.add_use_requirement(r_contract, ProgramPointId::from_raw(1), None);

    let solution = solve_region_graph(&graph);
    let result = realize_regions(&solution, &facts);

    assert!(result.is_err());
    match result.unwrap_err() {
        RealizationError::UninstantiatedAbstractRegion { region, kind, .. } => {
            assert_eq!(region, r_contract);
            assert_eq!(kind, RegionKind::Contract);
        }
        other => panic!("Expected UninstantiatedAbstractRegion, got {:?}", other),
    }
}

#[test]
fn contract_02_uninstantiated_external_region_rejected() {
    let mut graph = RegionGraph::new();
    let r_ext = graph.add_region(RegionKind::External { boundary: 0 }).unwrap();

    let mut facts = RealizationFacts::new();
    facts.add_use_requirement(r_ext, ProgramPointId::from_raw(1), None);

    let solution = solve_region_graph(&graph);
    let result = realize_regions(&solution, &facts);

    assert!(result.is_err());
    match result.unwrap_err() {
        RealizationError::UninstantiatedAbstractRegion { region, kind, .. } => {
            assert_eq!(region, r_ext);
            assert_eq!(kind, RegionKind::External { boundary: 0 });
        }
        other => panic!("Expected UninstantiatedAbstractRegion, got {:?}", other),
    }
}
