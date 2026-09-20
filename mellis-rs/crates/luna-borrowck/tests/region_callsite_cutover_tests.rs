//! Acceptance Test Matrix for REGION-02A: Call-Site Outlives Authority Cutover.
//!
//! Verifies:
//! - CALLSITE-01: Reordering unrelated ValueIds MUST NOT change contract satisfaction.
//! - CALLSITE-02: Scope containment evaluated strictly via structural Region facts (outer >= inner), not ValueId numerical order.
//! - CALLSITE-03: Caller contract assumptions (p0 >= p1 >= p2) satisfy callee obligations via transitive DAG reachability.
//! - CALLSITE-04: Callee obligation is queried against immutable RegionSolution and NEVER inserted into caller graph.
//! - CALLSITE-05: Direct contract violation produces canonical E2016 diagnostic with exact format and span.
//! - CALLSITE-06: Method receiver subject mapping (SelfVal >= Param(0)) in CallVirt.
//! - CALLSITE-07: Conservative Cartesian check for multi-provenance arguments (all pairs must satisfy; partial rejected).
//! - CALLSITE-08: Static/global operand (Program region) outlives parameters and locals; reverse fails.
//! - CALLSITE-09: Return relation (Param >= Return, life_from) is a callee guarantee and NOT checked as a call precondition.
//! - CALLSITE-10: Unresolved provenance mapping produces explicit Incomplete(ShadowGap), never silent pass or fallback.

use std::collections::HashMap;
use luna_borrowck::borrow_analysis::{BorrowAnalyzer, BorrowStateData, Loan};
use luna_borrowck::cfg::LoopInfo;
use luna_borrowck::{
    borrow_check_function_with_shadow, ContractViolationReason, RegionBorrowBridge,
    RegionBorrowContext, RegionFailure, ShadowGap, ShadowRegionVerdict,
};
use luna_common::{DiagnosticCode, FileId, Span};
use luna_mvir::*;
use luna_semantic::region::{realize_regions, solve_region_graph, LifetimeSubject};
use luna_semantic::{
    CanonicalLifetimeContract, CanonicalOutlivesConstraint, CanonicalProvenance,
    SemanticContext, SemanticType, SemanticTypeId,
};

fn make_test_function(name: &str) -> Function {
    Function {
        name: GlobalId {
            name: name.to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        values: Vec::new(),
        blocks: Vec::new(),
    }
}

fn make_block(name: &str, insts: Vec<ValueId>, term: Option<Terminator>) -> BasicBlock {
    BasicBlock {
        label: LabelId {
            name: name.to_string(),
        },
        insts,
        terminator: term,
    }
}

/// CALLSITE-01: Reordering unrelated ValueIds MUST NOT change contract satisfaction.
/// Proves ValueId numerical order is NEVER used as a lifetime relation.
#[test]
fn callsite_01_reordering_unrelated_values_preserves_satisfaction() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Register callee: fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter)
    let callee_sym = ctx.symbol_table.declare_symbol(
        "callee".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    let callee_contract = CanonicalLifetimeContract::new(
        None,
        vec![CanonicalOutlivesConstraint::new(0, 1)],
    );
    ctx.tables.fn_lifetime_contracts.insert(callee_sym, callee_contract);

    // Register caller: fn caller(a: &i32, b: &i32) where outlives(a, b)
    let caller_sym = ctx.symbol_table.declare_symbol(
        "caller".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    let caller_contract = CanonicalLifetimeContract::new(
        None,
        vec![CanonicalOutlivesConstraint::new(0, 1)],
    );
    ctx.tables.fn_lifetime_contracts.insert(caller_sym, caller_contract);

    // Variation A: Minimal values (p0=0, p1=1, call=2)
    let mut func_a = make_test_function("caller");
    func_a.name.symbol_id = Some(caller_sym);
    func_a.arg_count = 2;
    func_a.param_types = vec![ref_ty, ref_ty];
    // v0: p0
    func_a.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    // v1: p1
    func_a.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(1), inst: Instruction::Alloca, ty: ref_ty });
    // v2: call callee(p0, p1)
    func_a.values.push(ValueData {
        span: Some(Span::new(FileId(0), 10, 20)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(0)), Operand::Value(ValueId(1))],
        },
        ty: i32_ty,
    });
    func_a.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2)], Some(Terminator::Ret { value: None })));

    let diags_a = BorrowAnalyzer::analyze_with_shadow(&func_a, None, Some(&ctx)).0;
    assert!(diags_a.is_empty(), "Variation A: Valid callsite must pass! Got: {:?}", diags_a);

    // Variation B: Unrelated locals and arithmetic inserted before the call
    let mut func_b = make_test_function("caller");
    func_b.name.symbol_id = Some(caller_sym);
    func_b.arg_count = 2;
    func_b.param_types = vec![ref_ty, ref_ty];
    // v0: p0
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    // v1: p1
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(1), inst: Instruction::Alloca, ty: ref_ty });
    // v2: unrelated const 100
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("100".to_string())), ty: i32_ty });
    // v3: unrelated alloca
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v4: store 100 into v3
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(3)), value: Operand::Value(ValueId(2)) }, ty: i32_ty });
    // v5: unrelated add
    func_b.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Add { left: Operand::Value(ValueId(2)), right: Operand::Value(ValueId(2)) }, ty: i32_ty });
    // v6: call callee(p0, p1) with unrelated values inserted
    func_b.values.push(ValueData {
        span: Some(Span::new(FileId(0), 10, 20)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(0)), Operand::Value(ValueId(1))],
        },
        ty: i32_ty,
    });
    func_b.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6)], Some(Terminator::Ret { value: None })));

    let diags_b = BorrowAnalyzer::analyze_with_shadow(&func_b, None, Some(&ctx)).0;
    assert!(diags_b.is_empty(), "Variation B: Unrelated ValueIds must NOT affect contract satisfaction! Got: {:?}", diags_b);

    // Negative control: inverted call callee(p1, p0)
    let mut func_bad = func_b.clone();
    // replace v6 with call callee(p1, p0)
    func_bad.values[6] = ValueData {
        span: Some(Span::new(FileId(0), 10, 20)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(1)), Operand::Value(ValueId(0))],
        },
        ty: i32_ty,
    };
    let diags_bad = BorrowAnalyzer::analyze_with_shadow(&func_bad, None, Some(&ctx)).0;
    assert!(!diags_bad.is_empty(), "Inverted call MUST be rejected!");
    assert!(
        diags_bad.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Expected E2016 LifetimeConstraintViolation, got: {:?}",
        diags_bad
    );
}

/// CALLSITE-02: Scope containment evaluated strictly via structural Region facts (outer >= inner),
/// not ad-hoc ValueId numeric comparison.
#[test]
fn callsite_02_structural_scope_containment_not_value_id_order() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    let mut func = make_test_function("test_scope_containment");
    // v0: outer = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v1: 1
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("1".to_string())), ty: i32_ty });
    // v2: store 1 into outer
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(0)), value: Operand::Value(ValueId(1)) }, ty: i32_ty });
    // v3: inner = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v4: 2
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("2".to_string())), ty: i32_ty });
    // v5: store 2 into inner
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(3)), value: Operand::Value(ValueId(4)) }, ty: i32_ty });
    // v6: r_outer = Borrow &outer
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(0)), is_rw: false }, ty: ref_ty });
    // v7: r_inner = Borrow &inner
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(3)), is_rw: false }, ty: ref_ty });

    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6), ValueId(7)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let solution = solve_region_graph(&reg_ctx.graph);
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");

    let r_outer = *reg_ctx.val_to_region.get(&ValueId(0)).expect("outer region");
    let r_inner = *reg_ctx.val_to_region.get(&ValueId(3)).expect("inner region");

    // Structural containment invariant: R_outer ⪰ R_inner
    assert!(solution.outlives(r_outer, r_inner), "Structural invariant: outer must outlive inner in solution");
    assert!(!solution.outlives(r_inner, r_outer), "Structural invariant: inner must NOT outlive outer");

    let mut state = BorrowStateData::default();
    let loan_outer = Loan { id: ValueId(6), place: Operand::Value(ValueId(0)), is_rw: false };
    let loan_inner = Loan { id: ValueId(7), place: Operand::Value(ValueId(3)), is_rw: false };
    state.direct_provenance.entry(ValueId(6)).or_default().insert(loan_outer);
    state.direct_provenance.entry(ValueId(7)).or_default().insert(loan_inner);

    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);

    // callee(&outer, &inner) MUST BE VALID
    let verdict_valid = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(6)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(7)),
        &state,
        None,
    );
    assert_eq!(verdict_valid, ShadowRegionVerdict::Valid);

    // callee(&inner, &outer) MUST BE INVALID (E2016)
    let verdict_invalid = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(7)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(6)),
        &state,
        None,
    );
    assert!(matches!(
        verdict_invalid,
        ShadowRegionVerdict::Invalid(RegionFailure::UnsatisfiedContract {
            reason: ContractViolationReason::OutlivesPreconditionFailed { .. },
            ..
        })
    ));
}

/// CALLSITE-03: Caller contract assumptions (p0 >= p1 >= p2) satisfy callee obligations via transitive DAG reachability.
#[test]
fn callsite_03_caller_contract_assumptions_satisfy_callee_obligations_and_transitivity() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Callee: fn callee(x: &i32, y: &i32) where outlives(x, y)
    let callee_sym = ctx.symbol_table.declare_symbol(
        "callee".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    ctx.tables.fn_lifetime_contracts.insert(
        callee_sym,
        CanonicalLifetimeContract::new(None, vec![CanonicalOutlivesConstraint::new(0, 1)]),
    );

    // Caller: fn caller(a: &i32, b: &i32, c: &i32) where outlives(a, b) where outlives(b, c)
    let caller_sym = ctx.symbol_table.declare_symbol(
        "caller".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    ctx.tables.fn_lifetime_contracts.insert(
        caller_sym,
        CanonicalLifetimeContract::new(
            None,
            vec![
                CanonicalOutlivesConstraint::new(0, 1),
                CanonicalOutlivesConstraint::new(1, 2),
            ],
        ),
    );

    let mut func = make_test_function("caller");
    func.name.symbol_id = Some(caller_sym);
    func.arg_count = 3;
    func.param_types = vec![ref_ty, ref_ty, ref_ty];
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(1), inst: Instruction::Alloca, ty: ref_ty });
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(2), inst: Instruction::Alloca, ty: ref_ty });
    // v3: call callee(p0, p2) -> transitive outlives: p0 >= p2
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 10, 25)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(0)), Operand::Value(ValueId(2))],
        },
        ty: i32_ty,
    });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)], Some(Terminator::Ret { value: None })));

    let (diags, _, _) = borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());
    assert!(diags.is_empty(), "Transitive outlives call MUST pass! Got: {:?}", diags);

    // Negative control: inverted call callee(p2, p0)
    let mut func_bad = func.clone();
    func_bad.values[3] = ValueData {
        span: Some(Span::new(FileId(0), 10, 25)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(2)), Operand::Value(ValueId(0))],
        },
        ty: i32_ty,
    };
    let (diags_bad, _, _) = borrow_check_function_with_shadow(&func_bad, &ctx, &HashMap::new());
    assert!(!diags_bad.is_empty(), "Inverted transitive call MUST be rejected!");
    assert!(diags_bad.iter().any(|d| d.message.contains("E2016")));
}

/// CALLSITE-04: Callee obligation is queried against immutable RegionSolution and NEVER inserted into caller graph.
#[test]
fn callsite_04_callee_obligation_never_pollutes_caller_graph() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Caller has 2 unconstrained parameters (no contract)
    let mut func = make_test_function("unconstrained_caller");
    func.arg_count = 2;
    func.param_types = vec![ref_ty, ref_ty];
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(1), inst: Instruction::Alloca, ty: ref_ty });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let initial_constraint_count = reg_ctx.graph.constraints().len();

    let solution = solve_region_graph(&reg_ctx.graph);
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");

    let r_p0 = *reg_ctx.val_to_region.get(&ValueId(0)).expect("p0 region");
    let r_p1 = *reg_ctx.val_to_region.get(&ValueId(1)).expect("p1 region");

    // Before query: p0 does not outlive p1
    assert!(!solution.outlives(r_p0, r_p1), "Unconstrained parameters cannot outlive each other");

    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);
    let state = BorrowStateData::default();

    // Query callee obligation: p0 >= p1
    let verdict = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(0)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(1)),
        &state,
        None,
    );
    assert!(matches!(verdict, ShadowRegionVerdict::Invalid(_)));

    // Invariant: Callee obligation was NEVER inserted into the caller graph
    assert_eq!(
        reg_ctx.graph.constraints().len(),
        initial_constraint_count,
        "Querying callee obligation MUST NOT add any constraints to the caller graph"
    );
    // Solution remains immutable
    assert!(!solution.outlives(r_p0, r_p1), "Solution outlives relation MUST remain false");
}

/// CALLSITE-05: Direct contract violation produces canonical E2016 diagnostic with exact format and span.
#[test]
fn callsite_05_direct_contract_violation_emits_canonical_e2016() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Callee requires parameter 0 outlives parameter 1
    let callee_sym = ctx.symbol_table.declare_symbol(
        "constrained_callee".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    ctx.tables.fn_lifetime_contracts.insert(
        callee_sym,
        CanonicalLifetimeContract::new(None, vec![CanonicalOutlivesConstraint::new(0, 1)]),
    );

    let mut func = make_test_function("caller_with_violation");
    func.arg_count = 2;
    func.param_types = vec![ref_ty, ref_ty];
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(1), inst: Instruction::Alloca, ty: ref_ty });
    let call_span = Some(Span::new(FileId(42), 100, 120));
    func.values.push(ValueData {
        span: call_span.clone(),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "constrained_callee".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(0)), Operand::Value(ValueId(1))],
        },
        ty: i32_ty,
    });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2)], Some(Terminator::Ret { value: None })));

    let (diags, _, _) = borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());
    assert_eq!(diags.len(), 1, "Expected exactly 1 diagnostic, got: {:?}", diags);

    let diag = &diags[0];
    assert_eq!(diag.code, Some(DiagnosticCode::LifetimeConstraintViolation));
    assert_eq!(diag.span, call_span, "Diagnostic span must match call instruction span");
    assert_eq!(
        diag.message,
        "error[E2016]: LifetimeConstraintViolation: argument for parameter (index 0) does not outlive parameter (index 1)"
    );
}

/// CALLSITE-06: Method receiver subject mapping (SelfVal >= Param(0)) in CallVirt.
#[test]
fn callsite_06_method_receiver_subject_mapping() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_i32_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Register trait and method with contract: SelfVal >= Param(0)
    let trait_sym = ctx.symbol_table.declare_symbol(
        "MyTrait".to_string(),
        luna_semantic::symbol::SymbolKind::Trait,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    let method_sym = ctx.symbol_table.declare_symbol(
        "my_method".to_string(),
        luna_semantic::symbol::SymbolKind::TraitMethod,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    ctx.tables.trait_methods.insert(trait_sym, vec![method_sym]);
    // In canonical contract: longer = SelfVal, shorter = Param(0) (first explicit param)
    ctx.tables.fn_lifetime_contracts.insert(
        method_sym,
        CanonicalLifetimeContract::new(
            None,
            vec![CanonicalOutlivesConstraint::new(
                luna_semantic::CanonicalContractSubject::SelfVal,
                luna_semantic::CanonicalContractSubject::Param(0),
            )],
        ),
    );

    let dyn_trait_ty = ctx.types.intern(SemanticType::DynTrait(trait_sym));
    let ref_dyn_trait_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        dyn_trait_ty,
    ));

    // Caller with parameter p (long lifetime) and local obj (short lifetime)
    let mut func = make_test_function("caller_callvirt");
    func.arg_count = 1;
    func.param_types = vec![ref_i32_ty];
    // v0: p (Parameter 0)
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_i32_ty });
    // v1: local_obj (Local)
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: ref_dyn_trait_ty });
    // v2: init value
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("0".to_string())), ty: ref_dyn_trait_ty });
    // v3: store init into local_obj
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(1)), value: Operand::Value(ValueId(2)) }, ty: ref_dyn_trait_ty });
    // v4: CallVirt obj.my_method(p) where receiver is local (short) and arg is parameter (long) -> VIOLATION
    let call_span = Some(Span::new(FileId(0), 50, 70));
    func.values.push(ValueData {
        span: call_span.clone(),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallVirt {
            obj: Operand::Value(ValueId(1)),
            method_idx: 0,
            args: vec![Operand::Value(ValueId(0))],
        },
        ty: i32_ty,
    });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)], Some(Terminator::Ret { value: None })));

    let (diags, _, _) = borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());
    let lt_diag = diags.iter().find(|d| d.code == Some(DiagnosticCode::LifetimeConstraintViolation));
    assert!(lt_diag.is_some(), "Expected LifetimeConstraintViolation in diags: {:?}", diags);
    let diag = lt_diag.unwrap();
    assert_eq!(diag.span, call_span, "Diagnostic span must match call instruction span");
    assert_eq!(
        diag.message,
        "error[E2016]: LifetimeConstraintViolation: argument for receiver 'self' does not outlive parameter (index 0)"
    );
}

/// CALLSITE-07: Conservative Cartesian check for multi-provenance arguments.
/// Proves that when correlation is lost, ALL pairs must satisfy the constraint; partial satisfaction is rejected.
#[test]
fn callsite_07_conservative_cartesian_check_for_multi_provenance() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    let mut func = make_test_function("test_multi_provenance");
    // v0: outer = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v1: middle = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v2: inner = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v3: joined_carrier = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: ref_ty });
    // v4: middle_carrier = Alloca
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: ref_ty });
    // v5: drop inner early
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Drop { value: Operand::Value(ValueId(2)), ty: i32_ty, callee: None }, ty: i32_ty });
    // v6: drop middle
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Drop { value: Operand::Value(ValueId(1)), ty: i32_ty, callee: None }, ty: i32_ty });

    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let solution = solve_region_graph(&reg_ctx.graph);
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");

    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);

    // Scenario A: Multi-provenance joined_carrier has loans { outer, inner }.
    // target has loan { middle }.
    // Region ordering: R_outer ⪰ R_middle ⪰ R_inner.
    // Checking joined_carrier >= middle:
    // Pair (outer, middle): outlives = true.
    // Pair (inner, middle): outlives = false!
    // Cartesian check MUST fail!
    let mut state = BorrowStateData::default();
    let loan_outer = Loan { id: ValueId(0), place: Operand::Value(ValueId(0)), is_rw: false };
    let loan_middle = Loan { id: ValueId(1), place: Operand::Value(ValueId(1)), is_rw: false };
    let loan_inner = Loan { id: ValueId(2), place: Operand::Value(ValueId(2)), is_rw: false };

    // joined_carrier (v3) has { outer, inner }
    state.direct_provenance.entry(ValueId(3)).or_default().insert(loan_outer.clone());
    state.direct_provenance.entry(ValueId(3)).or_default().insert(loan_inner.clone());
    // middle_carrier (v4) has { middle }
    state.direct_provenance.entry(ValueId(4)).or_default().insert(loan_middle.clone());

    let verdict_fail = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(3)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(4)),
        &state,
        None,
    );
    assert!(
        matches!(verdict_fail, ShadowRegionVerdict::Invalid(_)),
        "Partial satisfaction with failing branch MUST be rejected by Cartesian check!"
    );

    // Scenario B: outer_carrier has { outer }.
    // multi_target has { middle, inner }.
    // Checking outer_carrier >= multi_target:
    // Pair (outer, middle): outlives = true.
    // Pair (outer, inner): outlives = true.
    // All pairs satisfy -> Cartesian check MUST pass!
    let mut state_ok = BorrowStateData::default();
    // v3 has { outer }
    state_ok.direct_provenance.entry(ValueId(3)).or_default().insert(loan_outer);
    // v4 has { middle, inner }
    state_ok.direct_provenance.entry(ValueId(4)).or_default().insert(loan_middle);
    state_ok.direct_provenance.entry(ValueId(4)).or_default().insert(loan_inner);

    let verdict_ok = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(3)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(4)),
        &state_ok,
        None,
    );
    assert_eq!(
        verdict_ok,
        ShadowRegionVerdict::Valid,
        "Universal satisfaction across all multi-provenance pairs MUST be valid!"
    );
}

/// CALLSITE-08: Static/global operand (Program region) outlives parameters and locals; reverse fails.
#[test]
fn callsite_08_static_global_outlives_parameters_and_locals() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    let mut func = make_test_function("test_global_outlives");
    func.arg_count = 1;
    func.param_types = vec![ref_ty];
    // v0: p (Parameter 0)
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    // v1: local
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let solution = solve_region_graph(&reg_ctx.graph);
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");

    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);
    let state = BorrowStateData::default();

    let global_op = Operand::Global(GlobalId {
        name: "GLOBAL_DATA".to_string(),
        symbol_id: None,
    });
    let param_op = Operand::Value(ValueId(0));
    let local_op = Operand::Value(ValueId(1));

    // Case 1: Global >= Param -> VALID
    let v_g_p = bridge.check_call_outlives(&LifetimeSubject::param(0), &global_op, &LifetimeSubject::param(1), &param_op, &state, None);
    assert_eq!(v_g_p, ShadowRegionVerdict::Valid, "Global must outlive parameter");

    // Case 2: Global >= Local -> VALID
    let v_g_l = bridge.check_call_outlives(&LifetimeSubject::param(0), &global_op, &LifetimeSubject::param(1), &local_op, &state, None);
    assert_eq!(v_g_l, ShadowRegionVerdict::Valid, "Global must outlive local");

    // Case 3: Local >= Global -> INVALID
    let v_l_g = bridge.check_call_outlives(&LifetimeSubject::param(0), &local_op, &LifetimeSubject::param(1), &global_op, &state, None);
    assert!(matches!(v_l_g, ShadowRegionVerdict::Invalid(_)), "Local cannot outlive global");

    // Case 4: Param >= Global -> INVALID
    let v_p_g = bridge.check_call_outlives(&LifetimeSubject::param(0), &param_op, &LifetimeSubject::param(1), &global_op, &state, None);
    assert!(matches!(v_p_g, ShadowRegionVerdict::Invalid(_)), "Parameter cannot outlive global");
}

/// CALLSITE-09: Return relation (Param >= Return, life_from) is a callee guarantee and NOT checked as a call precondition.
#[test]
fn callsite_09_return_relation_not_treated_as_call_precondition() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Callee: fn callee(x: &i32) -> &i32 life_from(x)
    // Contract has return_provenance = Some(Param(0)), but ZERO input outlives constraints!
    let callee_contract = CanonicalLifetimeContract::new(
        Some(CanonicalProvenance::Param(0)),
        Vec::new(),
    );

    // Assert that instantiate_call_preconditions yields zero obligations!
    let obligations = callee_contract.instantiate_call_preconditions(false);
    assert!(
        obligations.is_empty(),
        "Callee return relations MUST NOT generate call-site obligations! Got: {:?}",
        obligations
    );

    let callee_sym = ctx.symbol_table.declare_symbol(
        "callee_with_return_life".to_string(),
        luna_semantic::symbol::SymbolKind::Function,
        luna_semantic::symbol::ScopeId(0),
        Span::default(),
        None,
        luna_ast::Visibility::Public,
        &mut Vec::new(),
    );
    ctx.tables.fn_lifetime_contracts.insert(callee_sym, callee_contract);

    // Caller calling callee(p0)
    let mut func = make_test_function("caller");
    func.arg_count = 1;
    func.param_types = vec![ref_ty];
    func.values.push(ValueData { span: None, origin: ValueOrigin::Parameter(0), inst: Instruction::Alloca, ty: ref_ty });
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 10, 20)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: GlobalId { name: "callee_with_return_life".to_string(), symbol_id: Some(callee_sym) },
            args: vec![Operand::Value(ValueId(0))],
        },
        ty: ref_ty,
    });
    func.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1)], Some(Terminator::Ret { value: None })));

    let (diags, _, _) = borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());
    assert!(
        diags.is_empty(),
        "Calling a function with return life_from guarantee MUST NOT fail precondition checking! Got: {:?}",
        diags
    );
}

/// CALLSITE-10: Unresolved provenance mapping produces explicit Incomplete(ShadowGap), never silent pass or fallback.
#[test]
fn callsite_10_unresolved_provenance_mapping_produces_explicit_gap() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_gap");
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    func.blocks.push(make_block("entry", vec![ValueId(0)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let solution = solve_region_graph(&reg_ctx.graph);
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");

    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);
    let state = BorrowStateData::default();

    // ValueId(9999) does not exist in func or state
    let unmapped_val = Operand::Value(ValueId(9999));
    let resolved_regs = bridge.resolve_argument_dependency_regions(&unmapped_val, &state);
    assert_eq!(
        resolved_regs,
        Err(ShadowGap::UnmappedVariable(ValueId(9999))),
        "Unmapped variable must produce explicit ShadowGap::UnmappedVariable"
    );

    let verdict = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &unmapped_val,
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(0)),
        &state,
        None,
    );
    assert_eq!(
        verdict,
        ShadowRegionVerdict::Incomplete(ShadowGap::UnmappedVariable(ValueId(9999))),
        "Unmapped operand must result in explicit Incomplete(ShadowGap), never Valid or false fallback"
    );
}

/// CALLSITE-11: Structural relation is extent-derived, not declaration-order heuristic.
///
/// Proves:
/// 1. Changing declaration / ValueId ordering without changing actual validity-domain containment
///    does NOT change the verdict.
/// 2. Conversely, changing actual validity-domain containment (e.g. early drop) MUST change
///    the outlives relation even when declaration order remains unchanged.
#[test]
fn callsite_11_structural_relation_is_extent_derived() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    // Part 1: b is declared first, but a drops early, so extent(a) ⊂ extent(b).
    // b strictly outlives a despite reverse declaration order or arbitrary ValueId.
    let mut func_part1 = make_test_function("part1_extent_containment");
    // v0: b (outer local, lives throughout block)
    func_part1.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v1: a (inner local, dropped early at v3)
    func_part1.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v2: borrow &a
    func_part1.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(1)), is_rw: false }, ty: ref_ty });
    // v3: explicit drop of a
    func_part1.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Drop { value: Operand::Value(ValueId(1)), ty: i32_ty, callee: None }, ty: i32_ty });
    // v4: borrow &b (b is still alive!)
    func_part1.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(0)), is_rw: false }, ty: ref_ty });
    func_part1.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)], Some(Terminator::Ret { value: None })));

    let loop_info = LoopInfo::default();
    let reg_ctx1 = RegionBorrowContext::build(&func_part1, &loop_info, Some(&ctx));
    let solution1 = solve_region_graph(&reg_ctx1.graph);
    let r_b = *reg_ctx1.val_to_region.get(&ValueId(0)).expect("r_b");
    let r_a = *reg_ctx1.val_to_region.get(&ValueId(1)).expect("r_a");

    // Invariant: extent(a) ⊂ extent(b) => r_b outlives r_a, and r_a does NOT outlive r_b
    assert!(solution1.outlives(r_b, r_a), "b's extent contains a's extent => b outlives a");
    assert!(!solution1.outlives(r_a, r_b), "a's extent is smaller => a does NOT outlive b");

    // Part 2: x is declared FIRST, but dropped early at v1!
    // y is declared SECOND at v2, and lives until v4.
    // Under declaration-order heuristic, someone might think x ⪰ y.
    // But extent(y) is NOT contained in extent(x), so x CANNOT outlive y!
    let mut func_part2 = make_test_function("part2_disjoint_extents");
    // v0: x (declared first)
    func_part2.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v1: explicit early drop of x! (x's storage ends here)
    func_part2.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Drop { value: Operand::Value(ValueId(0)), ty: i32_ty, callee: None }, ty: i32_ty });
    // v2: y (declared second)
    func_part2.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v3: borrow &x
    func_part2.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(0)), is_rw: false }, ty: ref_ty });
    // v4: borrow &y
    func_part2.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(2)), is_rw: false }, ty: ref_ty });
    func_part2.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)], Some(Terminator::Ret { value: None })));

    let reg_ctx2 = RegionBorrowContext::build(&func_part2, &loop_info, Some(&ctx));
    let solution2 = solve_region_graph(&reg_ctx2.graph);
    let r_x = *reg_ctx2.val_to_region.get(&ValueId(0)).expect("r_x");
    let r_y = *reg_ctx2.val_to_region.get(&ValueId(2)).expect("r_y");

    // Canonical Invariant: x dropped before y was even in scope => x does NOT outlive y!
    assert!(
        !solution2.outlives(r_x, r_y),
        "Early dropped variable x MUST NOT outlive subsequently declared variable y despite declaration order"
    );
}

/// CALLSITE-12: Equal concrete extents imply lifetime equivalence.
/// Given: extent(A) == extent(B)
/// Expected:
///   solution.outlives(A, B) == true
///   solution.outlives(B, A) == true
///   solution.equivalent(A, B) == true
#[test]
fn callsite_12_equal_concrete_extents_imply_lifetime_equivalence() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        i32_ty,
    ));

    let mut func = make_test_function("test_equal_extents");
    // v0: local a (alloca preamble)
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v1: local b (alloca preamble, same block)
    func.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: i32_ty });
    // v2: borrow &a
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(0)), is_rw: false }, ty: ref_ty });
    // v3: borrow &b
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(1)), is_rw: false }, ty: ref_ty });

    // Neither local has an explicit drop: both live across the block to function exit.
    // Structural invariant: extent(a) == extent(b)
    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        Some(Terminator::Ret { value: None }),
    ));

    let loop_info = LoopInfo::default();
    let reg_ctx = RegionBorrowContext::build(&func, &loop_info, Some(&ctx));
    let solution = solve_region_graph(&reg_ctx.graph);

    let r_a = *reg_ctx.val_to_region.get(&ValueId(0)).expect("r_a must exist");
    let r_b = *reg_ctx.val_to_region.get(&ValueId(1)).expect("r_b must exist");

    // Canonical check: distinct regions but identical concrete domains
    assert_ne!(r_a, r_b, "r_a and r_b must be distinct region entities");

    // Subset-or-equal property:
    // extent(b) ⊆ extent(a) => a ⪰ b
    // extent(a) ⊆ extent(b) => b ⪰ a
    // SCC collapse => a ≡ b
    assert!(
        solution.outlives(r_a, r_b),
        "Equal extents MUST satisfy solution.outlives(A, B) == true"
    );
    assert!(
        solution.outlives(r_b, r_a),
        "Equal extents MUST satisfy solution.outlives(B, A) == true"
    );
    assert!(
        solution.equivalent(r_a, r_b),
        "Equal extents MUST collapse into the same SCC: solution.equivalent(A, B) == true"
    );

    // Furthermore, check call-site outlives verification in both directions:
    let realization = realize_regions(&solution, &reg_ctx.facts).expect("Realization must succeed");
    let bridge = RegionBorrowBridge::new(&reg_ctx, &solution, &realization);
    let mut state = BorrowStateData::default();
    let loan_a = Loan {
        id: ValueId(0),
        place: Operand::Value(ValueId(0)),
        is_rw: false,
    };
    let loan_b = Loan {
        id: ValueId(1),
        place: Operand::Value(ValueId(1)),
        is_rw: false,
    };
    state.direct_provenance.entry(ValueId(2)).or_default().insert(loan_a);
    state.direct_provenance.entry(ValueId(3)).or_default().insert(loan_b);

    let verdict_ab = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(2)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(3)),
        &state,
        None,
    );
    assert_eq!(verdict_ab, ShadowRegionVerdict::Valid, "a >= b must be Valid");

    let verdict_ba = bridge.check_call_outlives(
        &LifetimeSubject::param(0),
        &Operand::Value(ValueId(3)),
        &LifetimeSubject::param(1),
        &Operand::Value(ValueId(2)),
        &state,
        None,
    );
    assert_eq!(verdict_ba, ShadowRegionVerdict::Valid, "b >= a must be Valid");
}


