use mellis_borrowck::effect::{AccessKind, CallEffectSummary};
use mellis_borrowck::effect_inference::EffectInference;
use mellis_borrowck::dataflow::{DataflowEngine, DataflowAnalysis};
use mellis_mvir::{ValueOrigin, BasicBlock, Function, GlobalId, Instruction, LabelId, Operand, Terminator, ValueData, ValueId};
use mellis_semantic::ty::{Mutability, SemanticType, SemanticTypeId};
use mellis_semantic::SemanticContext;
use proptest::prelude::*;
use std::collections::HashMap;

const MAX_BLOCKS: usize = 5;
const MAX_INSTS: usize = 5;

// We generate acyclic MVIR graphs and trace all abstract paths.
// For acyclic, branch targets must be strictly greater than current block index.

fn build_acyclic_func(
    ctx: &mut SemanticContext,
    num_blocks: usize,
    insts_per_block: Vec<Vec<u8>>,
    term_kinds: Vec<u8>,
) -> Function {
    let mut func = Function {
        name: GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        is_extern: false,
            is_async: false,
        ret_ty: SemanticTypeId(0),
        blocks: vec![],
        values: vec![],
    };

    let prim_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::ty::BuiltinType::I32));
    let ref_mut_ty = ctx.types.intern(SemanticType::Reference(mellis_semantic::ty::LifetimeId(0), Mutability::Mutable, prim_ty));
    let ptr_mut_ty = ctx.types.intern(SemanticType::Pointer(Mutability::Mutable, prim_ty));

    // Param 0 and 1
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Alloca, ty: ref_mut_ty });
    func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst: Instruction::Alloca, ty: ptr_mut_ty });

    let mut val_id_counter = 2;
    let mut inst_id_to_val_id = vec![];
    for b_idx in 0..num_blocks {
        let mut block_val_ids = vec![];
        if b_idx == 0 {
            block_val_ids.push(ValueId(0));
            block_val_ids.push(ValueId(1));
        }

        for &kind in &insts_per_block[b_idx] {
            let inst = match kind % 3 {
                0 => Instruction::Borrow { is_rw: true, base: Operand::Value(ValueId(0)) },
                1 => Instruction::Load { ptr: Operand::Value(ValueId((kind % 2) as u32)) },
                _ => Instruction::Store { ptr: Operand::Value(ValueId((kind % 2) as u32)), value: Operand::Value(ValueId(0)) },
            };
            func.values.push(ValueData { span: None, origin: ValueOrigin::Temporary, inst, ty: prim_ty });
            block_val_ids.push(ValueId(val_id_counter));
            val_id_counter += 1;
        }
        inst_id_to_val_id.push(block_val_ids);
    }

    for b_idx in 0..num_blocks {
        let label = LabelId { name: format!("bb{}", b_idx) };
        let mut block = BasicBlock {
            label,
            insts: inst_id_to_val_id[b_idx].clone(),
            terminator: None,
        };

        let term_kind = term_kinds[b_idx % term_kinds.len()];
        if b_idx == num_blocks - 1 {
            block.terminator = Some(Terminator::Ret { value: None });
        } else {
            // Acyclic branches
            let next_range = b_idx + 1..num_blocks;
            let range_len = next_range.end - next_range.start;
            let target1 = next_range.start + (term_kind as usize % range_len);
            
            if term_kind % 2 == 0 {
                block.terminator = Some(Terminator::Br {
                    target: LabelId { name: format!("bb{}", target1) },
                });
            } else {
                let target2 = next_range.start + ((term_kind / 2) as usize % range_len);
                block.terminator = Some(Terminator::CondBr {
                    condition: Operand::Value(ValueId(0)), // arbitrary
                    true_target: LabelId { name: format!("bb{}", target1) },
                    false_target: LabelId { name: format!("bb{}", target2) },
                });
            }
        }
        func.blocks.push(block);
    }
    
    func
}

// Concrete path tracer
fn trace_paths(
    func: &Function,
    block_idx: usize,
    mut current_effect: HashMap<u32, AccessKind>,
    all_path_effects: &mut Vec<HashMap<u32, AccessKind>>,
) {
    let block = &func.blocks[block_idx];
    
    // Accumulate effect
    for &val_id in &block.insts {
        let inst = &func.value(val_id).inst;
        match inst {
            Instruction::Load { ptr: Operand::Value(ValueId(arg)) } if *arg < 2 => {
                let e = current_effect.entry(*arg).or_insert(AccessKind::None);
                if *e < AccessKind::Read { *e = AccessKind::Read; }
            }
            Instruction::Store { ptr: Operand::Value(ValueId(arg)), .. } if *arg < 2 => {
                let e = current_effect.entry(*arg).or_insert(AccessKind::None);
                if *e < AccessKind::Write { *e = AccessKind::Write; }
            }
            _ => {}
        }
    }

    match &block.terminator.as_ref().unwrap() {
        Terminator::Ret { .. } => {
            all_path_effects.push(current_effect);
        }
        Terminator::Br { target } => {
            let next_idx = func.blocks.iter().position(|b| b.label.name == target.name).unwrap();
            trace_paths(func, next_idx, current_effect, all_path_effects);
        }
        Terminator::CondBr { true_target, false_target, .. } => {
            let true_idx = func.blocks.iter().position(|b| b.label.name == true_target.name).unwrap();
            let false_idx = func.blocks.iter().position(|b| b.label.name == false_target.name).unwrap();
            
            trace_paths(func, true_idx, current_effect.clone(), all_path_effects);
            trace_paths(func, false_idx, current_effect, all_path_effects);
        }
        _ => {}
    }
}

proptest! {
    #[test]
    fn fuzz_soundness_property_acyclic(
        inst_counts in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 1..=MAX_INSTS), 1..=MAX_BLOCKS),
        term_kinds in proptest::collection::vec(any::<u8>(), 1..=MAX_BLOCKS),
    ) {
        let num_blocks = inst_counts.len();
        let mut ctx = SemanticContext::new();
        let func = build_acyclic_func(&mut ctx, num_blocks, inst_counts, term_kinds);

        // 1. Run Dataflow to compute CallEffectSummary
        let mut inference = EffectInference::new(2, vec![ValueId(0), ValueId(1)], None);
        let block_states = DataflowEngine::run_forward(&func, &mut inference);
        
        let mut access_effects = HashMap::new();
        for i in 0..2 {
            let eff = inference.summary.args.get(i).map(|a| a.access.clone()).unwrap_or(AccessKind::None);
            access_effects.insert(i as u32, eff);
        }

        // 2. DFS Trace all concrete paths
        let mut all_path_effects = Vec::new();
        trace_paths(&func, 0, HashMap::new(), &mut all_path_effects);

        // 3. Soundness Assert: Summary >= every PathEffect
        for path in all_path_effects {
            for arg in 0..2 {
                let path_eff = path.get(&arg).cloned().unwrap_or(AccessKind::None);
                let sum_eff = access_effects.get(&arg).cloned().unwrap_or(AccessKind::None);
                assert!(
                    path_eff <= sum_eff, 
                    "Soundness violation for arg {}: path evaluated to {:?}, but dataflow summary reported {:?}",
                    arg, path_eff, sum_eff
                );
            }
        }
    }
}



