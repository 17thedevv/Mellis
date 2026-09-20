//! Integration & Shadow Validation Tests for REGION-01D-C: Borrowck Bridge.
//!
//! Verifies:
//! - SHADOW-01: NLL Early Death agreement
//! - SHADOW-02: Carrier Transfer (A -> B) provenance preservation
//! - SHADOW-03: Branch Join multi-provenance conservative union
//! - SHADOW-04: Loop boundaries (GAP-17) iteration-local borrow escape rejection
//! - SHADOW-05: Outer reference in loop allowed to cross break (BOUNDARY-01)
//! - SHADOW-06: Method receiver ordering
//! - SHADOW-11: Raw pointer boundary (no safe lifetime fact)
//! - SHADOW-12: Zero silent Incomplete (explicit gap assertion & zero unexplained divergence)

use std::collections::HashMap;
use luna_borrowck::cfg::LoopInfo;
use luna_borrowck::{
    borrow_check_function_with_shadow, DivergenceClassification, LegacyVerdict,
    RegionBorrowBridge, RegionBorrowContext, ShadowGap, ShadowRegionVerdict,
};
use luna_mvir::*;
use luna_semantic::{SemanticContext, SemanticType, SemanticTypeId};

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

#[test]
fn shadow_01_nll_early_death() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_nll_early_death");
    // v0: x = Alloca
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
    // v2: store 42 into x (initializes x)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: r = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: load *r
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(3)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    // NLL early death: valid use at v4, no diagnostics
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got: {:?}", diagnostics);
    assert!(!comparisons.is_empty());

    // Verify zero unexplained divergence
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence at {}: {:?}",
            comp.location,
            comp
        );
    }
}

#[test]
fn shadow_02_carrier_transfer() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_carrier_transfer");
    // v0: x = Alloca
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
    // v2: store 42 into x (initializes x)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: r1 = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: r2 = Assign(r1) (carrier transfer)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Value(ValueId(3))),
        ty,
    });
    // v5: load *r2
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(4)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "Expected no diagnostics, got: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_03_branch_join_multi_provenance() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let bool_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::Bool));

    let mut func = make_test_function("test_branch_join");
    // v0: x = Alloca
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: y = Alloca
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v2: 10
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("10".to_string())),
        ty,
    });
    // v3: store 10 into x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(2)),
        },
        ty,
    });
    // v4: store 10 into y
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(1)),
            value: Operand::Value(ValueId(2)),
        },
        ty,
    });
    // v5: cond
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Boolean(true)),
        ty: bool_ty,
    });
    // v6: r_left = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v7: r_right = Borrow &y
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v8: r_join = Assign
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Value(ValueId(6))),
        ty,
    });
    // v9: load *r_join
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(8)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5)],
        terminator: Some(Terminator::CondBr {
            condition: Operand::Value(ValueId(5)),
            true_target: LabelId {
                name: "b_left".to_string(),
            },
            false_target: LabelId {
                name: "b_right".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "b_left".to_string(),
        },
        insts: vec![ValueId(6)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "join".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "b_right".to_string(),
        },
        insts: vec![ValueId(7)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "join".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "join".to_string(),
        },
        insts: vec![ValueId(8), ValueId(9)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "Expected no diagnostics, got: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_04_loop_boundaries_local_borrow_escape_gap17() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let bool_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::Bool));

    let mut func = make_test_function("test_gap17_escape");
    // v0: outer_carrier = Alloca
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: 0
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Number("0".to_string())),
        ty,
    });
    // v2: store 0 into outer_carrier
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: cond
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Boolean(true)),
        ty: bool_ty,
    });
    // v4: x_local = Alloca (inside loop body)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v5: store 10 into x_local
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(4)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v6: r_local = Borrow &x_local
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(4)),
        },
        ty,
    });
    // v7: Store r_local into outer_carrier
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(6)),
        },
        ty,
    });
    // v8: load outer_carrier after loop
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(0)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "loop_head".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "loop_head".to_string(),
        },
        insts: Vec::new(),
        terminator: Some(Terminator::CondBr {
            condition: Operand::Value(ValueId(3)),
            true_target: LabelId {
                name: "loop_body".to_string(),
            },
            false_target: LabelId {
                name: "exit".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "loop_body".to_string(),
        },
        insts: vec![ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "loop_head".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "exit".to_string(),
        },
        insts: vec![ValueId(8)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    // Both legacy borrowck and shadow bridge MUST reject escaping local borrow!
    assert!(
        !diagnostics.is_empty(),
        "Escaping local borrow must be rejected by borrowck"
    );

    let boundary_comps: Vec<_> = comparisons
        .iter()
        .filter(|c| c.location.contains("edge_loop_body_loop_head"))
        .collect();

    assert!(
        !boundary_comps.is_empty(),
        "Dynamic loop back-edge boundary check must be recorded"
    );

    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_05_outer_ref_in_loop_allowed_to_cross_break() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let bool_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::Bool));

    let mut func = make_test_function("test_boundary_01_allowed");
    // v0: outer_x = Alloca in entry
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
    // v2: store 10 into outer_x (initializes outer_x)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: outer_r = Alloca in entry
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v4: init_borrow = Borrow &outer_x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v5: store init_borrow into outer_r (initializes outer_r)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(3)),
            value: Operand::Value(ValueId(4)),
        },
        ty,
    });
    // v6: cond
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Assign(Operand::Boolean(true)),
        ty: bool_ty,
    });
    // v7: r = Borrow &outer_x (inside loop body)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v8: store r into outer_r
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(3)),
            value: Operand::Value(ValueId(7)),
        },
        ty,
    });
    // v9: load outer_r after exit
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(3)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "loop_head".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "loop_head".to_string(),
        },
        insts: Vec::new(),
        terminator: Some(Terminator::CondBr {
            condition: Operand::Value(ValueId(6)),
            true_target: LabelId {
                name: "loop_body".to_string(),
            },
            false_target: LabelId {
                name: "exit".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "loop_body".to_string(),
        },
        insts: vec![ValueId(7), ValueId(8)],
        terminator: Some(Terminator::Br {
            target: LabelId {
                name: "exit".to_string(),
            },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "exit".to_string(),
        },
        insts: vec![ValueId(9)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    // Borrowing outer object in loop and breaking out is legally allowed!
    assert!(
        diagnostics.is_empty(),
        "Borrowing outer object across loop break must be allowed: {:?}",
        diagnostics
    );

    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_11_raw_pointer_boundary() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_raw_pointer_boundary");
    // v0: x = Alloca
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v1: raw ptr offset
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::PtrOffset {
            ptr: Operand::Value(ValueId(0)),
            offset: Operand::Number("4".to_string()),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty());
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_06_closure_invocation_vs_escape() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_closure_invocation");
    // v0: x = Alloca
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
    // v3: r = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: MakeClosure capturing v3
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::MakeClosure {
            func: luna_mvir::GlobalId {
                name: "closure_fn".to_string(),
                symbol_id: None,
            },
            env_ptr: Operand::Value(ValueId(0)),
            captures: vec![luna_mvir::CaptureInfo {
                symbol: luna_common::ids::SymbolId(1),
                source: ValueId(3),
                env_field: 0,
                mode: luna_semantic::CaptureMode::SharedBorrow,
                ty,
                env_ty: ty,
            }],
        },
        ty,
    });
    // v5: CallClosure in same scope
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallClosure {
            closure: Operand::Value(ValueId(4)),
            args: Vec::new(),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "In-scope closure invocation must be allowed: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_07_aggregate_field_carrier() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_aggregate_field_carrier");
    // v0: x = Alloca
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
    // v3: holder = Alloca
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Local,
        inst: Instruction::Alloca,
        ty,
    });
    // v4: r = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v5: field_ptr = FieldPtr(holder, 0)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::FieldPtr {
            base: Operand::Value(ValueId(3)),
            field_idx: 0,
        },
        ty,
    });
    // v6: store r into field_ptr
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(5)),
            value: Operand::Value(ValueId(4)),
        },
        ty,
    });
    // v7: load from field_ptr
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Load {
            ptr: Operand::Value(ValueId(5)),
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "Expected no diagnostics, got: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_08_method_receiver_ordering() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_receiver_ordering");
    // v0: obj = Alloca
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
    // v2: store 10 into obj
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Store {
            ptr: Operand::Value(ValueId(0)),
            value: Operand::Value(ValueId(1)),
        },
        ty,
    });
    // v3: recv_borrow = Borrow &obj
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: CallDirect with recv_borrow
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: luna_mvir::GlobalId {
                name: "method_fn".to_string(),
                symbol_id: None,
            },
            args: vec![Operand::Value(ValueId(3))],
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "Method receiver call must be allowed: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_09_llib_contract_parity() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_llib_contract_parity");
    // v0: x = Alloca
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
    // v3: r = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: CallDirect to external library provider function
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: luna_mvir::GlobalId {
                name: "std::core::identity".to_string(),
                symbol_id: None,
            },
            args: vec![Operand::Value(ValueId(3))],
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty(), "Library function call with contract must succeed: {:?}", diagnostics);
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_10_ffi_explicit_contracts() {
    let mut ctx = SemanticContext::new();
    let ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));

    let mut func = make_test_function("test_ffi_contracts");
    // v0: x = Alloca
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
    // v3: r = Borrow &x
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Borrow {
            is_rw: false,
            base: Operand::Value(ValueId(0)),
        },
        ty,
    });
    // v4: CallDirect to FFI extern function
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::CallDirect {
            callee: luna_mvir::GlobalId {
                name: "c_print_int".to_string(),
                symbol_id: None,
            },
            args: vec![Operand::Value(ValueId(3))],
        },
        ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _, comparisons) =
        borrow_check_function_with_shadow(&func, &ctx, &HashMap::new());

    assert!(diagnostics.is_empty());
    for comp in &comparisons {
        assert!(
            !matches!(comp.classification, DivergenceClassification::Unexplained { .. }),
            "Unexplained divergence: {:?}",
            comp
        );
    }
}

#[test]
fn shadow_12_zero_silent_incomplete() {
    let loop_info = LoopInfo::default();
    let mut func = make_test_function("test_incomplete");
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Alloca,
        ty: SemanticTypeId(0),
    });
    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let context = RegionBorrowContext::build(&func, &loop_info, None);
    let solution = luna_semantic::region::solve_region_graph(&context.graph);
    let realization =
        luna_semantic::region::realize_regions(&solution, &context.facts).unwrap();
    let bridge = RegionBorrowBridge::new(&context, &solution, &realization);

    // Unsupported operand (e.g. Block operand used as carrier provenance)
    let unsupported_op = Operand::Block(luna_mvir::BlockId(99));
    let verdict = bridge.check_carrier_use(
        ValueId(0),
        &[unsupported_op.clone()],
        ValueId(0),
    );

    // Assert: Incomplete is returned, NEVER silently converted to Valid!
    match &verdict {
        ShadowRegionVerdict::Incomplete(ShadowGap::UnsupportedOperand(op)) => {
            assert_eq!(op, &unsupported_op);
        }
        other => panic!("Expected Incomplete(UnsupportedOperand), got {:?}", other),
    }

    // Comparison classification must be KnownUnsupportedGap, not Agreement
    let comp = bridge.evaluate_comparison(
        "test_loc".to_string(),
        verdict,
        LegacyVerdict::Allowed,
    );

    match comp.classification {
        DivergenceClassification::KnownUnsupportedGap { .. } => {}
        other => panic!("Expected KnownUnsupportedGap, got {:?}", other),
    }
}
