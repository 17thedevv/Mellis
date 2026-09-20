//! Integration and unit verification tests for Luna Region IR Foundation (REGION-01A).
//!
//! Frozen Test Matrix (REGION-IR-01 .. REGION-IR-11).

use luna_common::ids::FileId;
use luna_common::Span;
use luna_semantic::region::{
    ConstraintBoundary, ConstraintKind, ConstraintOrigin, ConstraintTransport, RegionGraph,
    RegionGraphError, RegionId, RegionKind,
};

#[test]
fn region_ir_01_unique_program_region() {
    let mut graph = RegionGraph::new();
    let prog = graph.program_region();

    assert_eq!(prog.index(), 0);
    let node = graph.get_node(prog).expect("Program node must exist");
    assert_eq!(node.kind, RegionKind::Program);
    assert_eq!(graph.nodes().len(), 1);

    // Attempting to add another Program region must be rejected
    let err = graph.add_region(RegionKind::Program);
    assert_eq!(err, Err(RegionGraphError::DuplicateProgramRegion));
    assert_eq!(graph.nodes().len(), 1);
}

#[test]
fn region_ir_02_lexical_regions_stable_distinct_ids() {
    let mut graph = RegionGraph::new();

    let r1 = graph.add_region(RegionKind::Lexical { scope: 10 }).unwrap();
    let r2 = graph.add_region(RegionKind::Lexical { scope: 20 }).unwrap();
    let r3 = graph.add_region(RegionKind::Lexical { scope: 30 }).unwrap();

    assert_eq!(r1.index(), 1);
    assert_eq!(r2.index(), 2);
    assert_eq!(r3.index(), 3);

    assert_ne!(r1, r2);
    assert_ne!(r2, r3);
    assert_ne!(r1, r3);

    assert_eq!(
        graph.get_node(r1).unwrap().kind,
        RegionKind::Lexical { scope: 10 }
    );
    assert_eq!(
        graph.get_node(r2).unwrap().kind,
        RegionKind::Lexical { scope: 20 }
    );
    assert_eq!(
        graph.get_node(r3).unwrap().kind,
        RegionKind::Lexical { scope: 30 }
    );
}

#[test]
fn region_ir_03_iteration_is_static_family_identity() {
    let mut graph = RegionGraph::new();

    // Loop iteration represents the family boundary, not runtime iteration counts
    let loop_reg = graph.add_region(RegionKind::Iteration { loop_id: 42 }).unwrap();
    let node = graph.get_node(loop_reg).unwrap();
    assert_eq!(node.kind, RegionKind::Iteration { loop_id: 42 });

    // Contract region represents abstract API validity variable
    let contract_reg = graph.add_region(RegionKind::Contract).unwrap();
    let cnode = graph.get_node(contract_reg).unwrap();
    assert_eq!(cnode.kind, RegionKind::Contract);

    // Inference region represents local body reference validity variable
    let infer_reg = graph.add_region(RegionKind::Inference { variable: 1 }).unwrap();
    let inode = graph.get_node(infer_reg).unwrap();
    assert_eq!(inode.kind, RegionKind::Inference { variable: 1 });
}

#[test]
fn region_ir_04_direct_outlives_insertion() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let rb = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();

    let cid = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: ra, sub: rb },
            ConstraintOrigin::borrow(),
            None,
        )
        .unwrap();

    assert_eq!(cid.index(), 0);
    assert!(graph.has_direct_outlives(ra, rb));
    assert!(!graph.has_direct_outlives(rb, ra));
}

#[test]
fn region_ir_05_equality_normalization() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let rb = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();

    let (c1, c2) = graph
        .add_equality(
            ra,
            rb,
            ConstraintOrigin::explicit_contract(
                ConstraintTransport::Source,
                ConstraintBoundary::Normal,
            ),
            None,
        )
        .unwrap();

    assert_eq!(c1.index(), 0);
    assert_eq!(c2.index(), 1);
    assert_eq!(graph.constraints().len(), 2);


    // Bidirectional outlives edges exist directly
    assert!(graph.has_direct_outlives(ra, rb));
    assert!(graph.has_direct_outlives(rb, ra));
}

#[test]
fn region_ir_06_self_outlives_legal() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();

    // Reflexive edge: Ra >= Ra
    let res = graph.add_constraint(
        ConstraintKind::Outlives { sup: ra, sub: ra },
        ConstraintOrigin::internal(),
        None,
    );

    assert!(res.is_ok());
    assert!(graph.has_direct_outlives(ra, ra));
}

#[test]
fn region_ir_07_cycles_structurally_legal() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let rb = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();

    // Adding cycles must not trigger cycle error (cycle == equality in formal spec)
    assert!(graph
        .add_constraint(
            ConstraintKind::Outlives { sup: ra, sub: rb },
            ConstraintOrigin::borrow(),
            None
        )
        .is_ok());
    assert!(graph
        .add_constraint(
            ConstraintKind::Outlives { sup: rb, sub: ra },
            ConstraintOrigin::borrow(),
            None
        )
        .is_ok());

    assert!(graph.has_direct_outlives(ra, rb));
    assert!(graph.has_direct_outlives(rb, ra));
}

#[test]
fn region_ir_08_unknown_invalid_region_id_rejected() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();

    let unknown = RegionId::from_raw(999);

    let res1 = graph.add_constraint(
        ConstraintKind::Outlives { sup: unknown, sub: ra },
        ConstraintOrigin::borrow(),
        None,
    );
    assert_eq!(res1, Err(RegionGraphError::UnknownRegion(unknown)));

    let res2 = graph.add_constraint(
        ConstraintKind::Outlives { sup: ra, sub: unknown },
        ConstraintOrigin::borrow(),
        None,
    );
    assert_eq!(res2, Err(RegionGraphError::UnknownRegion(unknown)));

    // Graph-local contract: RegionId is graph-local by API contract.
    // Unknown/out-of-range IDs are rejected. (Cross-graph misuse is not guaranteed
    // to be dynamically distinguishable when indices happen to alias).
    let cross_id = RegionId::from_raw(50);
    assert_eq!(
        graph.add_constraint(
            ConstraintKind::Outlives {
                sup: cross_id,
                sub: ra
            },
            ConstraintOrigin::borrow(),
            None
        ),
        Err(RegionGraphError::UnknownRegion(cross_id))
    );
}

#[test]
fn region_ir_09_deterministic_duplicate_edge_indexing() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let rb = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();

    let dummy_span = Span::new(FileId(1), 10, 20);

    let c1 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: ra, sub: rb },
            ConstraintOrigin::life_from(
                ConstraintTransport::Source,
                ConstraintBoundary::Normal,
            ),
            Some(dummy_span),
        )
        .unwrap();

    let c2 = graph
        .add_constraint(
            ConstraintKind::Outlives { sup: ra, sub: rb },
            ConstraintOrigin::structural(),
            None,
        )
        .unwrap();

    assert_ne!(c1, c2);
    // Both constraints are preserved in full with distinct origins/spans
    assert_eq!(graph.constraints().len(), 2);
    assert_eq!(
        graph.constraints()[0].origin,
        ConstraintOrigin::life_from(
            ConstraintTransport::Source,
            ConstraintBoundary::Normal,
        )
    );
    assert_eq!(graph.constraints()[0].span, Some(dummy_span));
    assert_eq!(
        graph.constraints()[1].origin,
        ConstraintOrigin::structural()
    );
    assert_eq!(graph.constraints()[1].span, None);

    // Derived adjacency index deduplicates to exactly one target
    let targets = graph.direct_outlives_targets(ra).unwrap();
    assert_eq!(targets.len(), 1);
    assert!(targets.contains(&rb));
}

#[test]
fn region_ir_10_origin_and_span_preserved() {
    let mut graph = RegionGraph::new();
    let ra = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let rb = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();
    let span = Span::new(FileId(5), 100, 150);

    let origins = [
        ConstraintOrigin::explicit_contract(
            ConstraintTransport::Source,
            ConstraintBoundary::Normal,
        ),
        ConstraintOrigin::life_from(
            ConstraintTransport::Source,
            ConstraintBoundary::Normal,
        ),
        ConstraintOrigin::elision(),
        ConstraintOrigin::borrow(),
        ConstraintOrigin::closure_capture(),
        ConstraintOrigin::structural(),
        ConstraintOrigin::explicit_contract(
            ConstraintTransport::Source,
            ConstraintBoundary::Ffi,
        ),
        ConstraintOrigin::explicit_contract(
            ConstraintTransport::Artifact,
            ConstraintBoundary::Normal,
        ),
        ConstraintOrigin::internal(),
    ];


    for (i, origin) in origins.into_iter().enumerate() {
        let cid = graph
            .add_constraint(
                ConstraintKind::Outlives { sup: ra, sub: rb },
                origin,
                Some(span),
            )
            .unwrap();

        assert_eq!(cid.index(), i as u32);
        let stored = &graph.constraints()[i];
        assert_eq!(stored.id, cid);
        assert_eq!(stored.origin, origin);
        assert_eq!(stored.span, Some(span));
        assert_eq!(
            stored.kind,
            ConstraintKind::Outlives { sup: ra, sub: rb }
        );
    }
}

#[test]
fn region_ir_11_no_transitive_solver_semantics() {
    let mut graph = RegionGraph::new();
    let a = graph.add_region(RegionKind::Lexical { scope: 1 }).unwrap();
    let b = graph.add_region(RegionKind::Lexical { scope: 2 }).unwrap();
    let c = graph.add_region(RegionKind::Lexical { scope: 3 }).unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: a, sub: b },
            ConstraintOrigin::borrow(),
            None,
        )
        .unwrap();

    graph
        .add_constraint(
            ConstraintKind::Outlives { sup: b, sub: c },
            ConstraintOrigin::borrow(),
            None,
        )
        .unwrap();


    // Direct edges are verified
    assert!(graph.has_direct_outlives(a, b));
    assert!(graph.has_direct_outlives(b, c));

    // CRITICAL: 01A does NOT compute transitive closure (A >= C).
    // Transitive reasoning belongs strictly to REGION-01C solver.
    assert!(!graph.has_direct_outlives(a, c));
}
