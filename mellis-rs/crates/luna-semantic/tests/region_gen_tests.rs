//! Integration and unit verification tests for Luna Region Constraint Generation (REGION-01B).
//!
//! Frozen Test Matrix (REGION-GEN-01 .. REGION-GEN-17 + REGION-GEN-PARITY-01).

use luna_common::ids::FileId;
use luna_common::Span;
use luna_semantic::region::{
    BodyConstraintGenerator, ConstraintBoundary, ConstraintRule, ConstraintTransport,
    ContractConstraintGenerator, LifetimeRegionBindings, LifetimeSubject, LifetimeSubjectRoot,
    RegionGenError, RegionGraph, RegionKind, ResolvedFunctionContract,
};

#[test]
fn region_gen_01_single_life_from() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    let contract = ResolvedFunctionContract::new().with_return_source(LifetimeSubject::param(0));

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            false,
            &[LifetimeSubject::param(0)],
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    let r_param = bindings.get(&LifetimeSubject::param(0)).unwrap();
    let r_return = bindings.get(&LifetimeSubject::return_val()).unwrap();

    // Invariant: Source outlives Return (R_param ⪰ R_return)
    assert!(graph.has_direct_outlives(r_param, r_return));

    let stored = &graph.constraints()[cids[0].index() as usize];
    assert_eq!(stored.origin.rule, ConstraintRule::LifeFrom);
    assert_eq!(stored.origin.transport, ConstraintTransport::Source);
    assert_eq!(stored.origin.boundary, ConstraintBoundary::Normal);
}

#[test]
fn region_gen_02_intersection_life_from() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 2, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // fn choose(a: &T, b: &T) -> &T life_from(a, b)
    let contract = ResolvedFunctionContract::new()
        .with_return_source(LifetimeSubject::param(0))
        .with_return_source(LifetimeSubject::param(1));

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            false,
            &[LifetimeSubject::param(0), LifetimeSubject::param(1)],
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 2);
    let r_a = bindings.get(&LifetimeSubject::param(0)).unwrap();
    let r_b = bindings.get(&LifetimeSubject::param(1)).unwrap();
    let r_return = bindings.get(&LifetimeSubject::return_val()).unwrap();

    // Both sources outlive return: R_a ⪰ R_return ∧ R_b ⪰ R_return
    assert!(graph.has_direct_outlives(r_a, r_return));
    assert!(graph.has_direct_outlives(r_b, r_return));
}

#[test]
fn region_gen_03_explicit_requires() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 2, false, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // requires life(a) >= life(b)
    let contract = ResolvedFunctionContract::new()
        .with_outlives(LifetimeSubject::param(0), LifetimeSubject::param(1));

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            false,
            &[LifetimeSubject::param(0), LifetimeSubject::param(1)],
            false,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    let r_a = bindings.get(&LifetimeSubject::param(0)).unwrap();
    let r_b = bindings.get(&LifetimeSubject::param(1)).unwrap();

    // Invariant: R_a ⪰ R_b
    assert!(graph.has_direct_outlives(r_a, r_b));
    let stored = &graph.constraints()[cids[0].index() as usize];
    assert_eq!(stored.origin.rule, ConstraintRule::ExplicitContract);
}

#[test]
fn region_gen_04_single_input_lle_elision() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // fn identity(x: &T) -> &T (no explicit contract)
    let contract = ResolvedFunctionContract::new();

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            false,
            &[LifetimeSubject::param(0)],
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    let r_x = bindings.get(&LifetimeSubject::param(0)).unwrap();
    let r_return = bindings.get(&LifetimeSubject::return_val()).unwrap();

    // Elision: R_x ⪰ R_return with Rule::Elision
    assert!(graph.has_direct_outlives(r_x, r_return));
    let stored = &graph.constraints()[cids[0].index() as usize];
    assert_eq!(stored.origin.rule, ConstraintRule::Elision);
}

#[test]
fn region_gen_05_struct_holder_invariant() {
    let mut graph = RegionGraph::new();
    let mut bindings = LifetimeRegionBindings::new();

    let r_field = graph.add_region(RegionKind::Contract).unwrap();
    let r_self = graph.add_region(RegionKind::Contract).unwrap();

    let field_sub = LifetimeSubject::field(LifetimeSubjectRoot::SelfVal, 0);
    let self_sub = LifetimeSubject::self_val();

    bindings.bind(field_sub.clone(), r_field).unwrap();
    bindings.bind(self_sub.clone(), r_self).unwrap();

    let contract_gen = ContractConstraintGenerator::new();
    let cids = contract_gen
        .lower_type_contract(
            &mut graph,
            &bindings,
            &[field_sub],
            &self_sub,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    // Invariant: Stored value must outlive struct instance (R_field ⪰ R_self)
    assert!(graph.has_direct_outlives(r_field, r_self));
    let stored = &graph.constraints()[cids[0].index() as usize];
    assert_eq!(stored.origin.rule, ConstraintRule::ExplicitContract);
}

#[test]
fn region_gen_06_reference_creation_shared_and_mut() {
    let mut graph = RegionGraph::new();
    let mut body_gen = BodyConstraintGenerator::new();

    let r_referent = body_gen.generate_lexical_region(&mut graph, 10).unwrap();
    let r_shared_ref = body_gen.generate_inference_region(&mut graph).unwrap();
    let r_mut_ref = body_gen.generate_inference_region(&mut graph).unwrap();

    let span = Span::new(FileId(1), 5, 15);

    // dec r = &x;
    let c1 = body_gen
        .generate_borrow_constraint(&mut graph, r_referent, r_shared_ref, Some(span))
        .unwrap();

    // dec r = &rw x;
    let c2 = body_gen
        .generate_borrow_constraint(&mut graph, r_referent, r_mut_ref, Some(span))
        .unwrap();

    // Both shared and mutable reference creation generate identical R_referent ⪰ R_ref
    assert!(graph.has_direct_outlives(r_referent, r_shared_ref));
    assert!(graph.has_direct_outlives(r_referent, r_mut_ref));

    assert_eq!(graph.constraints()[c1.index() as usize].origin.rule, ConstraintRule::Borrow);
    assert_eq!(graph.constraints()[c2.index() as usize].origin.rule, ConstraintRule::Borrow);
}

#[test]
fn region_gen_07_closure_capture_invariant() {
    let mut graph = RegionGraph::new();
    let mut body_gen = BodyConstraintGenerator::new();

    let r_captured = body_gen.generate_lexical_region(&mut graph, 10).unwrap();
    let r_closure = body_gen.generate_inference_region(&mut graph).unwrap();

    body_gen
        .generate_closure_capture_constraint(&mut graph, r_captured, r_closure, None)
        .unwrap();

    // Invariant: Captured provenance must outlive closure (R_captured ⪰ R_closure)
    assert!(graph.has_direct_outlives(r_captured, r_closure));
    assert!(!graph.has_direct_outlives(r_closure, r_captured));
}

#[test]
fn region_gen_08_negative_raw_pointer_rule() {
    let graph = RegionGraph::new();
    let body_gen = BodyConstraintGenerator::new();

    let initial_constraints = graph.constraints().len();
    body_gen.observe_raw_pointer_operation();

    // Raw pointer existence does not generate safe region constraints
    assert_eq!(graph.constraints().len(), initial_constraints);
}

#[test]
fn region_gen_09_three_axis_origin_preservation() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    let contract = ResolvedFunctionContract::new().with_return_source(LifetimeSubject::param(0));
    let span = Span::new(FileId(2), 50, 80);

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            false,
            &[LifetimeSubject::param(0)],
            true,
            ConstraintTransport::Source,
            Some(span),
        )
        .unwrap();

    let stored = &graph.constraints()[cids[0].index() as usize];
    assert_eq!(stored.origin.rule, ConstraintRule::LifeFrom);
    assert_eq!(stored.origin.transport, ConstraintTransport::Source);
    assert_eq!(stored.origin.boundary, ConstraintBoundary::Normal);
    assert_eq!(stored.span, Some(span));
}

#[test]
fn region_gen_10_explicit_return_contract_suppresses_lle_while_unrelated_does_not() {
    let contract_gen = ContractConstraintGenerator::new();

    // Case A: Explicit return contract suppresses LLE
    {
        let mut graph = RegionGraph::new();
        let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();

        let contract = ResolvedFunctionContract::new().with_return_source(LifetimeSubject::param(0));
        let cids = contract_gen
            .lower_function_contract(
                &mut graph,
                &bindings,
                &contract,
                false,
                &[LifetimeSubject::param(0)],
                true,
                ConstraintTransport::Source,
                None,
            )
            .unwrap();

        // Exactly 1 constraint (LifeFrom), NO duplicate Elision constraint
        assert_eq!(cids.len(), 1);
        assert_eq!(graph.constraints()[0].origin.rule, ConstraintRule::LifeFrom);
    }

    // Case B: Unrelated requires constraint does NOT suppress return LLE
    {
        let mut graph = RegionGraph::new();
        let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 3, true, false).unwrap();

        // requires life(param1) >= life(param2) - unrelated to Return
        let contract = ResolvedFunctionContract::new()
            .with_outlives(LifetimeSubject::param(1), LifetimeSubject::param(2));

        // Only param0 is an input reference that can flow to return
        let cids = contract_gen
            .lower_function_contract(
                &mut graph,
                &bindings,
                &contract,
                false,
                &[LifetimeSubject::param(0)],
                true,
                ConstraintTransport::Source,
                None,
            )
            .unwrap();

        // Expect 2 constraints: ExplicitContract (param1 >= param2) AND Elision (param0 >= return)
        assert_eq!(cids.len(), 2);
        assert_eq!(graph.constraints()[0].origin.rule, ConstraintRule::ExplicitContract);
        assert_eq!(graph.constraints()[1].origin.rule, ConstraintRule::Elision);
    }
}

#[test]
fn region_gen_11_no_lle_for_extern_fn() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // extern fn f(x: &T) -> &T (no explicit contract, is_extern = true)
    let contract = ResolvedFunctionContract::new();

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            true, // is_extern
            &[LifetimeSubject::param(0)],
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    // Invariant: FFI boundaries NEVER apply LLE
    assert!(cids.is_empty());
    assert!(graph.constraints().is_empty());
}

#[test]
fn region_gen_12_raw_pointer_explicit_ffi_contract() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // extern fn get(p: *T) -> &T requires life(p) >= life(return)
    // Author explicitly asserts the trust boundary relation
    let contract = ResolvedFunctionContract::new()
        .with_outlives(LifetimeSubject::param(0), LifetimeSubject::return_val());

    let cids = contract_gen
        .lower_function_contract(
            &mut graph,
            &bindings,
            &contract,
            true, // is_extern
            &[],   // no safe input references
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    let r_p = bindings.get(&LifetimeSubject::param(0)).unwrap();
    let r_return = bindings.get(&LifetimeSubject::return_val()).unwrap();

    assert!(graph.has_direct_outlives(r_p, r_return));
    let stored = &graph.constraints()[0];
    assert_eq!(stored.origin.rule, ConstraintRule::ExplicitContract);
    assert_eq!(stored.origin.boundary, ConstraintBoundary::Ffi);
}

#[test]
fn region_gen_13_semantic_identity_resolution() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 3, true, true).unwrap();

    // Resolve subjects through strongly-typed identities, not strings
    let r_self = bindings.require(&LifetimeSubject::self_val()).unwrap();
    let r_p0 = bindings.require(&LifetimeSubject::param(0)).unwrap();
    let r_p1 = bindings.require(&LifetimeSubject::param(1)).unwrap();
    let r_p2 = bindings.require(&LifetimeSubject::param(2)).unwrap();
    let r_ret = bindings.require(&LifetimeSubject::return_val()).unwrap();

    assert_ne!(r_self, r_p0);
    assert_ne!(r_p0, r_p1);
    assert_ne!(r_p1, r_p2);
    assert_ne!(r_p2, r_ret);
}

#[test]
fn region_gen_14_local_reference_uses_inference_region() {
    let mut graph = RegionGraph::new();
    let mut body_gen = BodyConstraintGenerator::new();

    let r1 = body_gen.generate_inference_region(&mut graph).unwrap();
    let r2 = body_gen.generate_inference_region(&mut graph).unwrap();

    assert_ne!(r1, r2);
    let node1 = graph.get_node(r1).unwrap();
    let node2 = graph.get_node(r2).unwrap();

    assert!(matches!(node1.kind, RegionKind::Inference { variable: 0 }));
    assert!(matches!(node2.kind, RegionKind::Inference { variable: 1 }));
}

#[test]
fn region_gen_15_projected_subject_identity_preserved() {
    let mut graph = RegionGraph::new();
    let mut bindings = LifetimeRegionBindings::new();

    let sub1 = LifetimeSubject::field(LifetimeSubjectRoot::Param(0), 1);
    let sub2 = LifetimeSubject::field(LifetimeSubjectRoot::Param(1), 1);
    let sub3 = LifetimeSubject::projection(LifetimeSubjectRoot::Param(0), vec![1, 2]);

    let r1 = graph.add_region(RegionKind::Contract).unwrap();
    let r2 = graph.add_region(RegionKind::Contract).unwrap();
    let r3 = graph.add_region(RegionKind::Contract).unwrap();

    bindings.bind(sub1.clone(), r1).unwrap();
    bindings.bind(sub2.clone(), r2).unwrap();
    bindings.bind(sub3.clone(), r3).unwrap();

    assert_ne!(sub1, sub2);
    assert_ne!(sub1, sub3);
    assert_eq!(bindings.require(&sub1).unwrap(), r1);
    assert_eq!(bindings.require(&sub2).unwrap(), r2);
    assert_eq!(bindings.require(&sub3).unwrap(), r3);
}

#[test]
fn region_gen_16_artifact_ffi_metadata_coexist() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::instantiate_function_bindings(&mut graph, 1, true, false).unwrap();
    let contract_gen = ContractConstraintGenerator::new();

    // Simulated .llib containing an extern fn with life_from(p)
    let contract = [(
        LifetimeSubject::param(0),
        LifetimeSubject::return_val(),
        ConstraintRule::LifeFrom,
    )];

    let cids = contract_gen
        .lower_canonical_contract(
            &mut graph,
            &bindings,
            &contract,
            ConstraintBoundary::Ffi,
            None,
        )
        .unwrap();

    assert_eq!(cids.len(), 1);
    let stored = &graph.constraints()[0];

    // All three axes coexist without mutual information loss
    assert_eq!(stored.origin.rule, ConstraintRule::LifeFrom);
    assert_eq!(stored.origin.transport, ConstraintTransport::Artifact);
    assert_eq!(stored.origin.boundary, ConstraintBoundary::Ffi);
}

#[test]
fn region_gen_17_structural_error_handling() {
    let mut graph = RegionGraph::new();
    let bindings = LifetimeRegionBindings::new(); // empty bindings
    let contract_gen = ContractConstraintGenerator::new();

    let contract = ResolvedFunctionContract::new().with_return_source(LifetimeSubject::param(0));

    // Attempting to lower when param(0) is unbound returns UnboundLifetimeSubject
    let err = contract_gen.lower_function_contract(
        &mut graph,
        &bindings,
        &contract,
        false,
        &[LifetimeSubject::param(0)],
        true,
        ConstraintTransport::Source,
        None,
    );

    assert!(matches!(
        err,
        Err(RegionGenError::UnboundLifetimeSubject(LifetimeSubject::Root(LifetimeSubjectRoot::Return)))
            | Err(RegionGenError::UnboundLifetimeSubject(LifetimeSubject::Root(LifetimeSubjectRoot::Param(0))))
    ));
}

#[test]
fn region_gen_parity_01_source_vs_llib_parity() {
    let contract_gen = ContractConstraintGenerator::new();

    // 1. Source compilation lowers fn f(x: &T) -> &T (elided to Param(0) >= Return)
    let mut source_graph = RegionGraph::new();
    let source_bindings =
        LifetimeRegionBindings::instantiate_function_bindings(&mut source_graph, 1, true, false).unwrap();

    let source_contract = ResolvedFunctionContract::new();
    contract_gen
        .lower_function_contract(
            &mut source_graph,
            &source_bindings,
            &source_contract,
            false,
            &[LifetimeSubject::param(0)],
            true,
            ConstraintTransport::Source,
            None,
        )
        .unwrap();

    // 2. Simulated .llib loading: carries canonical post-elision contract (Param(0) >= Return, Rule::Elision)
    let mut llib_graph = RegionGraph::new();
    let llib_bindings =
        LifetimeRegionBindings::instantiate_function_bindings(&mut llib_graph, 1, true, false).unwrap();

    let serialized_contract = [(
        LifetimeSubject::param(0),
        LifetimeSubject::return_val(),
        ConstraintRule::Elision,
    )];

    contract_gen
        .lower_canonical_contract(
            &mut llib_graph,
            &llib_bindings,
            &serialized_contract,
            ConstraintBoundary::Normal,
            None,
        )
        .unwrap();

    // 3. Parity verification: both graphs possess equivalent relation topology
    let src_p0 = source_bindings.require(&LifetimeSubject::param(0)).unwrap();
    let src_ret = source_bindings.require(&LifetimeSubject::return_val()).unwrap();
    assert!(source_graph.has_direct_outlives(src_p0, src_ret));

    let llib_p0 = llib_bindings.require(&LifetimeSubject::param(0)).unwrap();
    let llib_ret = llib_bindings.require(&LifetimeSubject::return_val()).unwrap();
    assert!(llib_graph.has_direct_outlives(llib_p0, llib_ret));

    // Both preserve rule = Elision
    assert_eq!(source_graph.constraints()[0].origin.rule, ConstraintRule::Elision);
    assert_eq!(llib_graph.constraints()[0].origin.rule, ConstraintRule::Elision);
}

#[test]
fn region_gen_18_structural_nesting_constraints() {
    let mut graph = RegionGraph::new();
    let body_gen = BodyConstraintGenerator::new();

    // 1. Lexical nesting: Router >= Rinner
    let r_outer = body_gen.generate_lexical_region(&mut graph, 1).unwrap();
    let r_inner = body_gen.generate_lexical_region(&mut graph, 2).unwrap();

    let cid_lex = body_gen
        .generate_lexical_nesting_constraint(&mut graph, r_outer, r_inner, None)
        .unwrap();

    assert!(graph.has_direct_outlives(r_outer, r_inner));
    let c_lex = graph.get_constraint(cid_lex).unwrap();
    assert_eq!(c_lex.origin.rule, ConstraintRule::Structural);
    assert_eq!(c_lex.origin.transport, ConstraintTransport::Synthesized);
    assert_eq!(c_lex.origin.boundary, ConstraintBoundary::Normal);

    // 2. Loop nesting: Renclosing >= Riteration
    let r_iter = body_gen.generate_loop_iteration_region(&mut graph, 10).unwrap();
    let cid_loop = body_gen
        .generate_loop_nesting_constraint(&mut graph, r_outer, r_iter, None)
        .unwrap();

    assert!(graph.has_direct_outlives(r_outer, r_iter));
    let c_loop = graph.get_constraint(cid_loop).unwrap();
    assert_eq!(c_loop.origin.rule, ConstraintRule::Structural);
    assert_eq!(c_loop.origin.transport, ConstraintTransport::Synthesized);
    assert_eq!(c_loop.origin.boundary, ConstraintBoundary::Normal);
}
