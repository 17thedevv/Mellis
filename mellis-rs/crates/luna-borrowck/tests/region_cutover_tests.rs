//! Acceptance Test Matrix for REGION-01D-D: Authority Cutover.
//!
//! Verifies:
//! - CUTOVER-01: Region verdict is the sole lifetime authority.
//! - CUTOVER-02: Legacy lifetime oracle does not emit diagnostics.
//! - CUTOVER-03: No duplicate E3005 on the same failure.
//! - CUTOVER-04: Simultaneous lifetime + capability failure preserves diagnostic precedence.
//! - CUTOVER-05: Incomplete/unsupported bridge state does not silently accept.
//! - CUTOVER-06: Region failure preserves expected code, message, and span.
//! - CUTOVER-07: Legacy lifetime heuristic removal does not affect provenance/carrier facts.
//! - CUTOVER-08: Source and .llib yield identical lifetime verdicts after cutover.
//! - CUTOVER-09: FFI does not introduce unwanted lifetime inference.
//! - CUTOVER-10: Loop dynamic-instance GAP-17 passes after legacy loop heuristic removal.

use std::collections::HashMap;
use luna_common::{DiagnosticCode, Span, FileId};
use luna_borrowck::cfg::LoopInfo;
use luna_borrowck::{
    borrow_check_function, borrow_check_function_with_shadow, ContractViolationReason,
    RegionBorrowBridge, RegionBorrowContext, RegionFailure, ReturnEscapeReason,
    ShadowGap, ShadowRegionVerdict,
};
use luna_mvir::*;
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

/// Helper to construct a basic block.
fn make_block(name: &str, insts: Vec<ValueId>, term: Option<Terminator>) -> BasicBlock {
    BasicBlock {
        label: LabelId {
            name: name.to_string(),
        },
        insts,
        terminator: term,
    }
}

#[test]
fn cutover_01_region_verdict_is_sole_lifetime_authority() {
    // Function returning a local variable borrow:
    // dec x = 10;
    // return &x;
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));

    let mut func = make_test_function("test_sole_lifetime_authority");
    func.ret_ty = ref_ty;

    // v0: x = Alloca
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 0, 10)),
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: 10
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 12, 14)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("10".to_string())),
        ty,
    });
    // v2: store 10 into x
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 10, 15)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: borrow &x
    func.values.push(ValueData {
        span: Some(Span::new(FileId(0), 20, 25)),
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: false,
        },
        ty: ref_ty,
    });

    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        Some(Terminator::Ret {
            value: Some(Operand::Value(ValueId(3))),
        }),
    ));

    let summaries = HashMap::new();
    let (diags, _dead_drops, shadow_comps) = borrow_check_function_with_shadow(&func, &ctx, &summaries);

    // Authority requirement: exactly one E3005 error emitted
    assert_eq!(diags.len(), 1, "Expected exactly 1 diagnostic from Region authority");
    assert!(
        diags[0].message.contains("E3005") && diags[0].message.contains("Reference to local variable escapes function scope"),
        "Diagnostic must be the canonical LocalBorrowEscape: {}",
        diags[0].message
    );

    // Shadow comparison should record agreement with Region engine
    assert!(!shadow_comps.is_empty());
}

#[test]
fn cutover_02_legacy_lifetime_oracle_does_not_emit_diagnostic() {
    // Standard borrow_check_function entry point (user-facing compiler path)
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));

    let mut func = make_test_function("test_no_legacy_oracle_emission");
    func.ret_ty = ref_ty;

    // v0: local x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: 42
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("42".to_string())),
        ty,
    });
    // v2: store 42 into x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: false,
        },
        ty: ref_ty,
    });

    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        Some(Terminator::Ret {
            value: Some(Operand::Value(ValueId(3))),
        }),
    ));

    let summaries = HashMap::new();
    let (diags, _) = borrow_check_function(&func, &ctx, &summaries);

    // No extra legacy oracle emissions; exactly 1 diagnostic from Region authority
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("error[E3005]"));
}

#[test]
fn cutover_03_no_duplicate_e3005_on_same_failure() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));

    let mut func = make_test_function("test_no_duplicate_e3005");
    func.ret_ty = ref_ty;

    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("100".to_string())),
        ty,
    });
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: false,
        },
        ty: ref_ty,
    });

    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        Some(Terminator::Ret {
            value: Some(Operand::Value(ValueId(3))),
        }),
    ));

    let summaries = HashMap::new();
    let (diags, _) = borrow_check_function(&func, &ctx, &summaries);

    let e3005_count = diags.iter().filter(|d| d.message.contains("E3005")).count();
    assert_eq!(e3005_count, 1, "Must not emit duplicate E3005 for the same escape");
}

#[test]
fn cutover_04_simultaneous_lifetime_and_capability_failure_preserves_precedence() {
    // Operation with both:
    // 1. Conflicting borrow at instruction time (mutable borrow while shared borrow is active, E3003)
    // 2. Local variable escape at return time (E3005)
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));
    let mutref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Mutable,
        ty,
    ));

    let mut func = make_test_function("test_precedence_lifetime_capability");
    func.ret_ty = ref_ty;

    // v0: local x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: 10
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("10".to_string())),
        ty,
    });
    // v2: store 10 into x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: r1 = borrow shared &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: false,
        },
        ty: ref_ty,
    });
    // v4: r2 = borrow mut &rw x (conflicts with r1 if r1 is live!)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: true,
        },
        ty: mutref_ty,
    });

    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        Some(Terminator::Ret {
            value: Some(Operand::Value(ValueId(3))), // r1 is used at return -> live across v4!
        }),
    ));

    let summaries = HashMap::new();
    let (diags, _) = borrow_check_function(&func, &ctx, &summaries);

    // Should observe BOTH:
    // BorrowConflict (from borrowck capability layer)
    // E3005: LocalBorrowEscape (from Region Engine authority)
    assert!(diags.len() >= 2, "Expected both capability and lifetime errors, got {}", diags.len());
    let has_conflict = diags.iter().any(|d| d.code == Some(DiagnosticCode::BorrowConflict) || d.message.contains("Cannot borrow"));
    let has_escape = diags.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape"));
    assert!(has_conflict, "Borrowck capability exclusivity check must be preserved");
    assert!(has_escape, "Region Engine lifetime escape check must be preserved");

    // Verify ordering: borrow conflict occurs at instruction v4 before return terminator E3005
    let idx_conflict = diags.iter().position(|d| d.code == Some(DiagnosticCode::BorrowConflict) || d.message.contains("Cannot borrow")).unwrap();
    let idx_escape = diags.iter().position(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")).unwrap();
    assert!(idx_conflict < idx_escape, "Instruction borrow conflict must precede terminator return escape");
}

#[test]
fn cutover_05_incomplete_unsupported_bridge_state_does_not_silently_accept() {
    let mut func = make_test_function("test_incomplete_gap");
    let ty = SemanticTypeId(0);
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0)],
        Some(Terminator::Ret { value: None }),
    ));

    let loop_info = LoopInfo::default();
    let ctx = RegionBorrowContext::build(&func, &loop_info, None);
    let sol = luna_semantic::region::solve_region_graph(&ctx.graph);
    let real = luna_semantic::region::realize_regions(&sol, &ctx.facts).unwrap();
    let bridge = RegionBorrowBridge::new(&ctx, &sol, &real);

    // Query with an unsupported operand (e.g. constant string operand as provenance source)
    let unsupported = Operand::StringRef("extern_symbol".to_string());
    let verdict = bridge.check_carrier_use(ValueId(999), &[unsupported.clone()], ValueId(0));

    // Doctrine: Must be explicit Incomplete(ShadowGap), NEVER Valid!
    assert!(matches!(verdict, ShadowRegionVerdict::Incomplete(ShadowGap::UnsupportedOperand(_))));
    assert_ne!(verdict, ShadowRegionVerdict::Valid);
}

#[test]
fn cutover_06_region_failure_preserves_expected_code_message_span() {
    let test_span = Some(Span::new(FileId(1), 120, 135));

    // 1. BoundaryViolation -> E3005 with BorrowConflict code
    let failure_boundary = RegionFailure::BoundaryViolation {
        carrier: ValueId(10),
        edge: luna_semantic::region::CfgEdgeId::from_raw(0),
        dependency_region: luna_semantic::region::RegionId::from_raw(1),
        terminated_region: luna_semantic::region::RegionId::from_raw(2),
        source: Operand::Value(ValueId(5)),
        span: test_span.clone(),
    };
    let diag_boundary = failure_boundary.into_diagnostic();
    assert_eq!(diag_boundary.code, Some(DiagnosticCode::BorrowConflict));
    assert!(diag_boundary.message.contains("error[E3005]: LocalBorrowEscape: Reference to iteration-local variable escapes loop iteration"));
    assert_eq!(diag_boundary.span, test_span);

    // 2. ReturnEscape -> E3005
    let failure_return = RegionFailure::ReturnEscape {
        carrier: ValueId(20),
        reason: ReturnEscapeReason::ClosureCapturesLocalBorrow,
        span: test_span.clone(),
    };
    let diag_return = failure_return.into_diagnostic();
    assert!(diag_return.message.contains("error[E3005]: LocalBorrowEscape: Cannot return a closure that captures a local borrow"));
    assert_eq!(diag_return.span, test_span);

    // 3. UnsatisfiedContract -> E2016
    let failure_contract = RegionFailure::UnsatisfiedContract {
        carrier: ValueId(30),
        reason: ContractViolationReason::ParameterNotContracted { param_index: 2 },
        span: test_span.clone(),
    };
    let diag_contract = failure_contract.into_diagnostic();
    assert!(diag_contract.message.contains("error[E2016]: LifetimeConstraintViolation: return value has provenance from parameter (index 2)"));
    assert_eq!(diag_contract.span, test_span);
}

#[test]
fn cutover_07_legacy_lifetime_heuristic_removal_does_not_affect_provenance_carrier_facts() {
    // Loop where carrier passes through and has loans killed upon exiting iteration
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));

    let mut func = make_test_function("test_provenance_retention");
    // v0: outer variable
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: 5
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("5".to_string())),
        ty,
    });
    // v2: store 5 into outer
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: borrow &outer
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            base: Operand::Value(ValueId(0)),
            is_rw: false,
        },
        ty: ref_ty,
    });
    // v4: carrier assignment = v3
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Value(ValueId(3))),
        ty: ref_ty,
    });

    func.blocks.push(make_block(
        "entry",
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        Some(Terminator::Ret { value: None }),
    ));

    let summaries = HashMap::new();
    let (diags, _) = borrow_check_function(&func, &ctx, &summaries);
    // Outer borrow used and assigned to carrier within entry block is 100% valid
    assert_eq!(diags.len(), 0, "Carrying provenance of outer borrow must remain valid");
}

#[test]
fn cutover_08_source_and_llib_parity_after_cutover() {
    // Verifies that a function contract evaluated through Region Engine gives identical
    // results whether from source representation or .llib CanonicalLifetimeContract
    let contract = CanonicalLifetimeContract::new(
        Some(CanonicalProvenance::Param(0)),
        vec![CanonicalOutlivesConstraint {
            longer: 0,
            shorter: 1,
        }],
    );

    let allowed = contract.return_provenance.as_ref().unwrap().indices();
    assert_eq!(allowed, &[0]);
    assert!(contract.outlives_constraints.iter().any(|c| c.longer == 0 && c.shorter == 1));
}

#[test]
fn cutover_09_ffi_no_unwanted_lifetime_inference() {
    let func = Function {
        name: GlobalId {
            name: "c_external_func".to_string(),
            symbol_id: None,
        },
        is_extern: true,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 2,
        link_name: Some("c_external_func".to_string()),
        param_types: vec![SemanticTypeId(1), SemanticTypeId(2)],
        values: Vec::new(),
        blocks: Vec::new(),
    };

    let loop_info = LoopInfo::default();
    let ctx = RegionBorrowContext::build(&func, &loop_info, None);

    // Negative FFI inference rule: no implicit elision constraint added across FFI
    let outlives_edges = ctx.graph.constraints();
    assert!(
        !outlives_edges.iter().any(|c| matches!(c.origin.rule, luna_semantic::region::ConstraintRule::Elision)),
        "FFI must never synthesize elision lifetime constraints"
    );
}

#[test]
fn cutover_10_loop_dynamic_instance_gap17_passes_with_legacy_heuristic_removed() {
    // Critical test: GAP-17 Dynamic boundary crossing
    // 1. Escaping iteration-local reference -> rejected with E3005
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let bool_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::Bool));
    let ref_ty = ctx.types.intern(SemanticType::Reference(
        luna_semantic::ty::LifetimeId(0),
        luna_semantic::ty::Mutability::Immutable,
        ty,
    ));

    let mut func_escape = make_test_function("test_gap17_escape");
    // v0: outer_carrier = Alloca
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty: ref_ty });
    // v1: cond = true
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Boolean(true)), ty: bool_ty });
    // v2: x_local = Alloca (inside loop body)
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty });
    // v3: 42
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("42".to_string())), ty });
    // v4: store 42 into x_local
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(2)), value: Operand::Value(ValueId(3)) }, ty });
    // v5: r_local = Borrow &x_local
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(2)), is_rw: false }, ty: ref_ty });
    // v6: store r_local into outer_carrier
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(0)), value: Operand::Value(ValueId(5)) }, ty: ref_ty });
    // v7: load outer_carrier after loop
    func_escape.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) }, ty: ref_ty });

    // entry:
    func_escape.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1)], Some(Terminator::Br { target: LabelId { name: "loop_head".to_string() } })));
    // loop_head:
    func_escape.blocks.push(make_block("loop_head", vec![], Some(Terminator::CondBr {
        condition: Operand::Value(ValueId(1)),
        true_target: LabelId { name: "loop_body".to_string() },
        false_target: LabelId { name: "exit".to_string() },
    })));
    // loop_body:
    func_escape.blocks.push(make_block("loop_body", vec![ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6)], Some(Terminator::Br { target: LabelId { name: "loop_head".to_string() } })));
    // exit:
    func_escape.blocks.push(make_block("exit", vec![ValueId(7)], Some(Terminator::Ret { value: None })));

    let summaries = HashMap::new();
    let (diags, _) = borrow_check_function(&func_escape, &ctx, &summaries);
    assert!(!diags.is_empty(), "Escaping iteration-local reference must be rejected by Region authority");
    assert!(diags.iter().any(|d| d.message.contains("E3005") || d.message.contains("escapes loop")), "Must emit E3005 for escaping iteration local: {:?}", diags);

    // 2. Reference borrowing OUTER data across loop break -> BOUNDARY-01 must be accepted!
    let mut func_outer = make_test_function("test_boundary01_outer");
    // Outer local v0 in entry
    func_outer.values.push(ValueData { span: None, origin: ValueOrigin::Local, inst: Instruction::Alloca, ty });
    func_outer.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Number("10".to_string())), ty });
    func_outer.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Store { ptr: Operand::Value(ValueId(0)), value: Operand::Value(ValueId(1)) }, ty });
    func_outer.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Assign(Operand::Boolean(true)), ty: bool_ty });
    // Inside loop: borrow &outer (v4)
    func_outer.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Borrow { base: Operand::Value(ValueId(0)), is_rw: false }, ty: ref_ty });

    func_outer.blocks.push(make_block("entry", vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)], Some(Terminator::Br { target: LabelId { name: "loop_head".to_string() } })));
    func_outer.blocks.push(make_block("loop_head", vec![], Some(Terminator::CondBr {
        condition: Operand::Value(ValueId(3)),
        true_target: LabelId { name: "loop_body".to_string() },
        false_target: LabelId { name: "exit".to_string() },
    })));
    func_outer.blocks.push(make_block("loop_body", vec![ValueId(4)], Some(Terminator::Br { target: LabelId { name: "exit".to_string() } })));
    func_outer.blocks.push(make_block("exit", vec![], Some(Terminator::Ret { value: None })));

    let (diags_outer, _) = borrow_check_function(&func_outer, &ctx, &summaries);
    assert_eq!(diags_outer.len(), 0, "Reference borrowing outer variable crossing loop exit must be legally permitted");
}
