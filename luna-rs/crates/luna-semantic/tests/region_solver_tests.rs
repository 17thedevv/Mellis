//! Verification Test Suite for REGION-01C: Symbolic Region Relation Solver.
//!
//! Tests the 18 core verification scenarios covering:
//! - Direct & transitive outlives entailment (SOLVE-01, 02)
//! - Disconnected reachability (SOLVE-03)
//! - Reflexivity & Program axiom (SOLVE-04, 08)
//! - Bidirectional equivalence & multi-node cycle normalization (SOLVE-05, 06)
//! - Condensation DAG acyclicity (SOLVE-07)
//! - Non-mutating derived query execution (SOLVE-09)
//! - Proof explanation & 3-axis metadata retention (SOLVE-10, 11)
//! - Static bound validation & negative bounds (SOLVE-12, 17)
//! - Disconnected components & determinism (SOLVE-13, 14)
//! - Program axiom in SCC equivalence (SOLVE-15, 16)
//! - ProgramAxiom proof step (SOLVE-18)

use luna_semantic::region::{
    solve_region_graph, ConstraintBoundary, ConstraintKind, ConstraintOrigin, ConstraintRule,
    ConstraintTransport, ProofStep, RegionGraph, RegionKind, StaticBound,
};

fn origin_contract() -> ConstraintOrigin {
    ConstraintOrigin::explicit_contract(ConstraintTransport::Source, ConstraintBoundary::Normal)
}

fn origin_life_from() -> ConstraintOrigin {
    ConstraintOrigin::life_from(ConstraintTransport::Source, ConstraintBoundary::Normal)
}

#[test]
fn solve_01_direct_outlives() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(solution.outlives(a, b));
    assert!(!solution.outlives(b, a));
    assert!(!solution.equivalent(a, b));
}

#[test]
fn solve_02_transitive_outlives() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(solution.outlives(a, b));
    assert!(solution.outlives(b, c));
    assert!(solution.outlives(a, c)); // Transitive entailment
    assert!(!solution.outlives(c, a));
}

#[test]
fn solve_03_disconnected_reachability() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(!solution.outlives(a, c));
    assert!(!solution.outlives(b, c));
    assert!(!solution.outlives(c, a));
}

#[test]
fn solve_04_self_relation_reflexive() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let program = graph.program_region();

    let solution = solve_region_graph(&graph);

    assert!(solution.outlives(a, a));
    assert!(solution.outlives(program, program));
    assert!(solution.equivalent(a, a));
    assert!(solution.equivalent(program, program));

    let proof = solution.explain_outlives(a, a).unwrap();
    assert_eq!(proof.steps, vec![ProofStep::Reflexive { region: a }]);
}

#[test]
fn solve_05_bidirectional_equality() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_equality(a, b, origin_contract(), None)
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(solution.equivalent(a, b));
    assert!(solution.outlives(a, b));
    assert!(solution.outlives(b, a));
    assert_eq!(solution.scc_of(a), solution.scc_of(b));
}

#[test]
fn solve_06_three_node_cycle_single_scc() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            origin_contract(),
            None,
        )
        .unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: c, sub: a },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(solution.equivalent(a, b));
    assert!(solution.equivalent(b, c));
    assert!(solution.equivalent(a, c));

    let scc_id = solution.scc_of(a).unwrap();
    let members = solution.scc_members(scc_id).unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.contains(&a));
    assert!(members.contains(&b));
    assert!(members.contains(&c));
}

#[test]
fn solve_07_scc_condensation_acyclic() {
    let mut graph = RegionGraph::new();
    // Component 1: A <-> B
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    graph
        .add_equality(a, b, origin_contract(), None)
        .unwrap();

    // Component 2: C <-> D
    let c = graph.add_region(RegionKind::Contract).unwrap();
    let d = graph.add_region(RegionKind::Contract).unwrap();
    graph
        .add_equality(c, d, origin_contract(), None)
        .unwrap();

    // Edge between components: B -> C
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            origin_contract(),
            None,
        )
        .unwrap();

    // Component 3: Single node E, Edge: D -> E
    let e = graph.add_region(RegionKind::Contract).unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: d, sub: e },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    // Verify condensation DAG has no cycles via DFS
    let num_sccs = solution.classes().len();
    let mut visited = vec![0u8; num_sccs]; // 0: unvisited, 1: in-progress, 2: finished

    fn dfs_cycle(u: usize, solution: &luna_semantic::region::RegionSolution<'_>, visited: &mut [u8]) -> bool {
        visited[u] = 1;
        if let Some(edges) = solution.condensation_edges(luna_semantic::region::SccId(u as u32)) {
            for &next in edges {
                let v = next.index() as usize;
                if visited[v] == 1 {
                    return true; // Cycle detected
                }
                if visited[v] == 0 && dfs_cycle(v, solution, visited) {
                    return true;
                }
            }
        }
        visited[u] = 2;
        false
    }

    for i in 0..num_sccs {
        if visited[i] == 0 {
            assert!(!dfs_cycle(i, &solution, &mut visited), "Condensation DAG must be strictly acyclic");
        }
    }
}

#[test]
fn solve_08_program_outlives_every_registered_region() {
    let mut graph = RegionGraph::new();
    let program = graph.program_region();
    let r_lex = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let r_iter = graph.add_region(RegionKind::Iteration { loop_id: 2 }).unwrap();
    let r_con = graph.add_region(RegionKind::Contract).unwrap();
    let r_inf = graph.add_region(RegionKind::Inference { variable: 3 }).unwrap();

    let solution = solve_region_graph(&graph);

    assert!(solution.outlives(program, r_lex));
    assert!(solution.outlives(program, r_iter));
    assert!(solution.outlives(program, r_con));
    assert!(solution.outlives(program, r_inf));
}

#[test]
fn solve_09_direct_vs_derived_non_mutating() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            origin_contract(),
            None,
        )
        .unwrap();

    let initial_constraints = graph.constraints().len();

    let solution = solve_region_graph(&graph);

    // Entailment is proven
    assert!(solution.outlives(a, c));

    // But canonical RegionGraph is NOT mutated with transitive edges
    assert_eq!(graph.constraints().len(), initial_constraints);
    assert!(!graph.has_direct_outlives(a, c));
}

#[test]
fn solve_10_proof_explanation_witness_path() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    let cid1 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();
    let cid2 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    let proof = solution.explain_outlives(a, c).unwrap();
    assert_eq!(proof.steps.len(), 2);
    assert_eq!(
        proof.steps[0],
        ProofStep::Direct {
            from: a,
            to: b,
            constraint_id: cid1,
        }
    );
    assert_eq!(
        proof.steps[1],
        ProofStep::Direct {
            from: b,
            to: c,
            constraint_id: cid2,
        }
    );

    // Intra-SCC witness path explanation
    let cid3 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: c, sub: a },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution_cycle = solve_region_graph(&graph);
    let proof_intra = solution_cycle.explain_outlives(a, c).unwrap();
    assert_eq!(proof_intra.steps.len(), 2);
    assert_eq!(proof_intra.steps[0], ProofStep::Direct { from: a, to: b, constraint_id: cid1 });
    assert_eq!(proof_intra.steps[1], ProofStep::Direct { from: b, to: c, constraint_id: cid2 });

    let proof_back = solution_cycle.explain_outlives(c, a).unwrap();
    assert_eq!(proof_back.steps.len(), 1);
    assert_eq!(proof_back.steps[0], ProofStep::Direct { from: c, to: a, constraint_id: cid3 });
}

#[test]
fn solve_11_metadata_retention_in_proof() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();

    let cid1 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_life_from(),
            None,
        )
        .unwrap();
    let cid2 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            ConstraintOrigin::borrow(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);
    let proof = solution.explain_outlives(a, c).unwrap();

    // Verify metadata retention through graph lookup
    let step0 = &proof.steps[0];
    let step1 = &proof.steps[1];

    if let ProofStep::Direct { constraint_id, .. } = step0 {
        assert_eq!(*constraint_id, cid1);
        let constraint = graph.get_constraint(*constraint_id).unwrap();
        assert_eq!(constraint.origin.rule, ConstraintRule::LifeFrom);
    } else {
        panic!("Expected Direct step");
    }

    if let ProofStep::Direct { constraint_id, .. } = step1 {
        assert_eq!(*constraint_id, cid2);
        let constraint = graph.get_constraint(*constraint_id).unwrap();
        assert_eq!(constraint.origin.rule, ConstraintRule::Borrow);
    } else {
        panic!("Expected Direct step");
    }
}

#[test]
fn solve_12_static_bound_contradiction() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    // Negative bound asserting that A cannot outlive B
    let bound = StaticBound::cannot_outlive(a, b, None);
    let result = solution.validate_static_bounds(&[bound]);

    assert!(result.is_err());
    let violation = result.unwrap_err();
    assert_eq!(violation.proof.steps.len(), 1);
}

#[test]
fn solve_13_unrelated_graph_components() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Contract).unwrap();
    let b = graph.add_region(RegionKind::Contract).unwrap();
    let c = graph.add_region(RegionKind::Contract).unwrap();
    let d = graph.add_region(RegionKind::Contract).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            origin_contract(),
            None,
        )
        .unwrap();
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: c, sub: d },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    assert!(!solution.outlives(a, c));
    assert!(!solution.outlives(a, d));
    assert!(!solution.outlives(b, c));
    assert!(!solution.outlives(b, d));
    assert!(!solution.outlives(c, a));
    assert!(!solution.outlives(d, a));
}

#[test]
fn solve_14_deterministic_scc_canonicalization() {
    let build_graph = || {
        let mut g = RegionGraph::new();
        let r1 = g.add_region(RegionKind::Contract).unwrap();
        let r2 = g.add_region(RegionKind::Contract).unwrap();
        let r3 = g.add_region(RegionKind::Contract).unwrap();
        let r4 = g.add_region(RegionKind::Contract).unwrap();

        // Cycle between r3 and r2
        g.add_equality(r3, r2, origin_contract(), None)
            .unwrap();
        // Edge from r1 to r3
        g.add_constraint(
            ConstraintKind::Outlives { sup: r1, sub: r3 },
            origin_contract(),
            None,
        )
        .unwrap();
        // Edge from r2 to r4
        g.add_constraint(
            ConstraintKind::Outlives { sup: r2, sub: r4 },
            origin_contract(),
            None,
        )
        .unwrap();

        g
    };

    let g1 = build_graph();
    let g2 = build_graph();

    let sol1 = solve_region_graph(&g1);
    let sol2 = solve_region_graph(&g2);

    assert_eq!(sol1.classes(), sol2.classes());
    for node in g1.nodes() {
        assert_eq!(sol1.scc_of(node.id), sol2.scc_of(node.id));
    }
}

#[test]
fn solve_15_program_axiom_participates_in_equivalence() {
    let mut graph = RegionGraph::new();
    let program = graph.program_region();
    let a = graph.add_region(RegionKind::Contract).unwrap();

    // A outlives Program
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: program },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    // Because Program >= A by axiom, A and Program form a cycle and collapse into the same SCC
    assert!(solution.equivalent(a, program));
    assert!(solution.outlives(a, program));
    assert!(solution.outlives(program, a));
    assert_eq!(solution.scc_of(a), solution.scc_of(program));
}

#[test]
fn solve_16_positive_promotion_not_inherently_invalid() {
    let mut graph = RegionGraph::new();
    let program = graph.program_region();
    let a = graph.add_region(RegionKind::Inference { variable: 0 }).unwrap();

    // Positive constraint promoting inference region to Program
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: program },
            origin_contract(),
            None,
        )
        .unwrap();

    // Solver succeeds without error when no negative bound is provided
    let solution = solve_region_graph(&graph);
    assert!(solution.equivalent(a, program));
}

#[test]
fn solve_17_fixed_lexical_promotion_rejected() {
    let mut graph = RegionGraph::new();
    let program = graph.program_region();
    let local = graph.add_region(RegionKind::Lexical { scope: 42 }).unwrap();

    // Simulated illegal promotion in constraint graph
    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: local, sub: program },
            origin_contract(),
            None,
        )
        .unwrap();

    let solution = solve_region_graph(&graph);

    // Negative bound: Local scope cannot outlive Program
    let bound = StaticBound::cannot_outlive(local, program, None);
    let result = solution.validate_static_bounds(&[bound]);

    assert!(result.is_err());
}

#[test]
fn solve_18_axiomatic_proof_explanation() {
    let mut graph = RegionGraph::new();
    let program = graph.program_region();
    let a = graph.add_region(RegionKind::Contract).unwrap();

    let solution = solve_region_graph(&graph);

    let proof = solution.explain_outlives(program, a).unwrap();
    assert_eq!(proof.steps, vec![ProofStep::ProgramAxiom { to: a }]);
}
