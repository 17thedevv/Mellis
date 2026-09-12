use luna_borrowck::borrow_check_function;
use luna_mvir::*;
use luna_semantic::{SemanticContext, SemanticTypeId, ty::{SemanticType, Mutability}};
use proptest::prelude::*;

// To avoid timeouts in large fuzzer tests, we cap the sizes.
const MAX_BLOCKS: usize = 5;
const MAX_INSTS: usize = 10;
const MAX_VARS: usize = 15;

#[derive(Debug, Clone)]
enum FuzzInst {
    Alloca,
    Borrow { is_rw: bool, base: u32 },
    Load { ptr: u32 },
    Store { ptr: u32, value: u32 },
    Call { callee: String, args: Vec<u32> },
}

fn arb_fuzz_inst(num_vars: u32) -> impl Strategy<Value = FuzzInst> {
    if num_vars == 0 {
        return Just(FuzzInst::Alloca).boxed();
    }
    
    let var_idx = 0..num_vars;
    
    prop_oneof![
        Just(FuzzInst::Alloca),
        (any::<bool>(), var_idx.clone()).prop_map(|(is_rw, base)| FuzzInst::Borrow { is_rw, base }),
        var_idx.clone().prop_map(|ptr| FuzzInst::Load { ptr }),
        (var_idx.clone(), var_idx.clone()).prop_map(|(ptr, value)| FuzzInst::Store { ptr, value }),
        (Just("opaque_func".to_string()), proptest::collection::vec(var_idx, 0..3)).prop_map(|(callee, args)| FuzzInst::Call { callee, args }),
    ].boxed()
}

fn arb_semantic_type(ctx: &mut SemanticContext) -> impl Strategy<Value = SemanticTypeId> {
    // We pre-intern a few types to use in fuzzing.
    let prim = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
    let ptr_mut = ctx.types.intern(SemanticType::Pointer(Mutability::Mutable, prim));
    let ptr_const = ctx.types.intern(SemanticType::Pointer(Mutability::Immutable, prim));
    let ref_mut = ctx.types.intern(SemanticType::Reference(luna_semantic::ty::LifetimeId(0), Mutability::Mutable, prim));
    
    prop_oneof![
        Just(prim),
        Just(ptr_mut),
        Just(ptr_const),
        Just(ref_mut),
    ]
}

proptest! {
    #[test]
    fn fuzz_borrowck_no_panic_and_converges(
        insts_per_block in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 1..=MAX_INSTS), 1..=MAX_BLOCKS),
        term_kinds_raw in proptest::collection::vec(any::<u8>(), 1..=MAX_BLOCKS),
    ) {
        let num_blocks = insts_per_block.len();
        let mut ctx = SemanticContext::new();
        
        // Setup types
        let prim = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::I32));
        let ptr_mut = ctx.types.intern(SemanticType::Pointer(Mutability::Mutable, prim));
        let ptr_const = ctx.types.intern(SemanticType::Pointer(Mutability::Immutable, prim));
        
        let mut func = Function {
        name: luna_mvir::GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        is_extern: false,
            is_async: false,
        
        blocks: vec![],
            values: vec![],
            ret_ty: prim,
        };
        
        let mut val_id_counter = 0;
        
        for i in 0..num_blocks {
            let label = LabelId { name: format!("bb{}", i) };
            let mut block = BasicBlock {
                label: label.clone(),
                insts: vec![],
                terminator: None,
            };
            
            let num_insts = insts_per_block[i].len();
            for j in 0..num_insts {
                let r = insts_per_block[i][j];
                
                let inst = if val_id_counter == 0 {
                    Instruction::Alloca
                } else {
                    let base = (r as u32) % val_id_counter;
                    let val2 = ((r as u32) / 2) % val_id_counter;
                    
                    match r % 5 {
                        0 => Instruction::Alloca,
                        1 => Instruction::Borrow { is_rw: (r % 2 == 0), base: Operand::Value(ValueId(base)) },
                        2 => Instruction::Load { ptr: Operand::Value(ValueId(base)) },
                        3 => Instruction::Store { ptr: Operand::Value(ValueId(base)), value: Operand::Value(ValueId(val2)) },
                        _ => Instruction::CallDirect {
                            callee: GlobalId { name: "opaque_func".to_string(), symbol_id: None },
                            args: vec![Operand::Value(ValueId(base))]
                        },
                    }
                };
                
                let ty = match r % 3 {
                    0 => ptr_mut,
                    1 => ptr_const,
                    _ => prim,
                };
                
                func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst, ty });
                block.insts.push(ValueId(val_id_counter));
                val_id_counter += 1;
            }
            
            // Terminator
            let term_kind = term_kinds_raw[i % term_kinds_raw.len()];
            let term = if i == num_blocks - 1 {
                Terminator::Ret { value: None }
            } else {
                match term_kind % 2 {
                    0 => Terminator::Br { target: LabelId { name: format!("bb{}", (i + 1) % num_blocks) } },
                    _ => Terminator::CondBr { 
                        condition: Operand::Value(ValueId(0)), 
                        true_target: LabelId { name: format!("bb{}", (i + 1) % num_blocks) }, 
                        false_target: LabelId { name: format!("bb{}", (i + 2) % num_blocks) } 
                    },
                }
            };
            
            block.terminator = Some(term);
            func.blocks.push(block);
        }
        
        // Just run borrow_check_function and ensure it doesn't panic and terminates
        use std::collections::HashMap;
        let summaries = HashMap::new();
        let _diags = borrow_check_function(&func, &ctx, &summaries);
    }
}



