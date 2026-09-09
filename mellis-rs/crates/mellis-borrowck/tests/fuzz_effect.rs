use mellis_borrowck::effect::*;
use mellis_borrowck::effect_inference::EffectInference;
use mellis_mvir::*;
use mellis_semantic::SemanticTypeId;
use proptest::prelude::*;

// Test 13: Lattice laws
proptest! {
    #[test]
    fn test_access_lattice_laws(a in 0..4, b in 0..4, c in 0..4) {
        let kinds = [AccessKind::None, AccessKind::Read, AccessKind::Write, AccessKind::ReadWrite, AccessKind::Unknown];
        let ka = &kinds[a as usize];
        let kb = &kinds[b as usize];
        let kc = &kinds[c as usize];

        // Idempotency: a ⊔ a = a
        assert_eq!(ka.merge(ka), *ka);

        // Commutativity: a ⊔ b = b ⊔ a
        assert_eq!(ka.merge(kb), kb.merge(ka));

        // Associativity: a ⊔ (b ⊔ c) = (a ⊔ b) ⊔ c
        assert_eq!(ka.merge(&kb.merge(kc)), ka.merge(kb).merge(kc));

        // Upper bound property: a <= a ⊔ b
        assert!(ka <= &ka.merge(kb));
        assert!(kb <= &ka.merge(kb));
    }
}

// Helper to generate a simple straight-line function
fn generate_linear_function(num_args: u32, ops: Vec<u8>) -> Function {
    let mut func = Function {
        name: GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: num_args as usize,
        link_name: None,
        param_types: vec![],
        is_extern: false,
            is_async: false,
        blocks: vec![],
        values: vec![],
        ret_ty: SemanticTypeId(0),
    };

    let mut block = BasicBlock {
        label: LabelId {
            name: "0".to_string(),
        },
        insts: vec![],
        terminator: Some(Terminator::Ret { value: None }),
    };

    // Push arguments
    for i in 0..num_args {
        func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary,
            inst: Instruction::Alloca,
            ty: SemanticTypeId(0),
        });
        block.insts.push(ValueId(i));
    }

    let mut val_idx = num_args;

    for op in ops {
        // Simplified ops for fuzzing:
        // 0: Load arg0
        // 1: Store to arg0
        // 2: Borrow arg0
        let inst = match op % 3 {
            0 => Instruction::Load {
                ptr: Operand::Value(ValueId(0)),
            },
            1 => Instruction::Store {
                ptr: Operand::Value(ValueId(0)),
                value: Operand::Number("1".to_string()),
            },
            2 => Instruction::Borrow {
                is_rw: true,
                base: Operand::Value(ValueId(0)),
            },
            _ => unreachable!(),
        };

        func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary,
            inst,
            ty: SemanticTypeId(0),
        });
        block.insts.push(ValueId(val_idx));
        val_idx += 1;
    }

    func.blocks.push(block);
    func
}

// Test 14: Monotonicity property
proptest! {
    #[test]
    fn test_monotonicity(ops in prop::collection::vec(0..3u8, 0..10), extra_op in 0..3u8) {
        let func1 = generate_linear_function(1, ops.clone());
        let summary1 = EffectInference::infer(&func1, vec![ValueId(0)], None);

        let mut ops2 = ops.clone();
        ops2.push(extra_op);
        let func2 = generate_linear_function(1, ops2);
        let summary2 = EffectInference::infer(&func2, vec![ValueId(0)], None);

        // Adding an instruction must monotonically increase or maintain the summary effects
        assert!(summary1 <= summary2);
    }
}

// Test 15: Random CFG upper-bound property (Simplified to linear path for testability)
proptest! {
    #[test]
    fn test_upper_bound_property(ops in prop::collection::vec(0..3u8, 0..10)) {
        let func = generate_linear_function(1, ops.clone());
        let summary = EffectInference::infer(&func, vec![ValueId(0)], None);

        // We calculate exact expected effects based on ops
        let mut expected_access = AccessKind::None;
        let mut expected_ownership = OwnershipKind::Copy;

        for op in ops {
            match op % 3 {
                0 => expected_access = expected_access.merge(&AccessKind::Read),
                1 => expected_access = expected_access.merge(&AccessKind::Write),
                2 => expected_ownership = expected_ownership.merge(&OwnershipKind::BorrowMut),
                _ => {}
            }
        }

        // summary must be AT LEAST the expected exact effects (soundness)
        assert!(summary.args[0].access >= expected_access);
        assert!(summary.args[0].ownership >= expected_ownership);
    }
}



