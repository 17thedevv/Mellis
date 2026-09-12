use luna_mvir::{Function, Instruction, Terminator, Operand};
use crate::pass::Pass;
use std::collections::HashSet;

pub struct DeadCodeElimination;

impl Pass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "DeadCodeElimination"
    }

    fn run_on_function(&mut self, func: &mut Function) -> bool {
        let mut changed = false;
        
        loop {
            let mut used_values = HashSet::new();
            
            // Collect uses from terminators
            for block in &func.blocks {
                if let Some(term) = &block.terminator {
                    match term {
                        Terminator::Ret { value: Some(Operand::Value(val_id)) } => {
                            used_values.insert(val_id.0);
                        }
                        Terminator::CondBr { condition: Operand::Value(val_id), .. } => {
                            used_values.insert(val_id.0);
                        }
                        _ => {}
                    }
                }
                
                // Collect uses from active instructions
                for inst_id in &block.insts {
                    let val = func.value(*inst_id);
                    match &val.inst {
                        Instruction::Store { ptr, value } => {
                            if let Operand::Value(v) = ptr { used_values.insert(v.0); }
                            if let Operand::Value(v) = value { used_values.insert(v.0); }
                        }
                        Instruction::Load { ptr } => {
                            if let Operand::Value(v) = ptr { used_values.insert(v.0); }
                        }
                        Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } | Instruction::Div { left, right } | Instruction::Rem { left, right } |
                        Instruction::Eq { left, right } | Instruction::NotEq { left, right } | Instruction::LessThan { left, right } | Instruction::LessOrEq { left, right } | Instruction::GreaterThan { left, right } | Instruction::GreaterOrEq { left, right } |
                        Instruction::BitAnd { left, right } | Instruction::BitOr { left, right } | Instruction::BitXor { left, right } |
                        Instruction::Shl { left, right } | Instruction::Shr { left, right } => {
                            if let Operand::Value(v) = left { used_values.insert(v.0); }
                            if let Operand::Value(v) = right { used_values.insert(v.0); }
                        }
                        Instruction::CallIntrinsic { args, .. } => {
                            for arg in args {
                                if let Operand::Value(v) = arg { used_values.insert(v.0); }
                            }
                        }
                        Instruction::CallDirect { args, .. } => {
                            for arg in args {
                                if let Operand::Value(v) = arg { used_values.insert(v.0); }
                            }
                        }
                        Instruction::CallIndirect { callee, args } | Instruction::CallClosure { closure: callee, args } => {
                            if let Operand::Value(v) = callee { used_values.insert(v.0); }
                            for arg in args {
                                if let Operand::Value(v) = arg { used_values.insert(v.0); }
                            }
                        }
                        Instruction::Borrow { base, .. } => {
                            if let Operand::Value(v) = base { used_values.insert(v.0); }
                        }
                        Instruction::Assign(Operand::Value(v)) => {
                            used_values.insert(v.0);
                        }
                        Instruction::Extract { value, .. } | Instruction::Tag { value } | Instruction::FieldPtr { base: value, .. } => {
                            if let Operand::Value(v) = value { used_values.insert(v.0); }
                        }
                        Instruction::Variant { args, .. } => {
                            for arg in args {
                                if let Operand::Value(v) = arg { used_values.insert(v.0); }
                            }
                        }
                        Instruction::BoundsCheck { index, len } => {
                            if let Operand::Value(v) = index { used_values.insert(v.0); }
                            if let Operand::Value(v) = len { used_values.insert(v.0); }
                        }
                        Instruction::CallVirt { obj, args, .. } => {
                            if let Operand::Value(v) = obj { used_values.insert(v.0); }
                            for arg in args {
                                if let Operand::Value(v) = arg { used_values.insert(v.0); }
                            }
                        }
                        Instruction::BoxNew { value } | Instruction::BoxFree { value } | Instruction::Drop { value, .. } | Instruction::DropVirt { obj: value } => {
                            if let Operand::Value(v) = value { used_values.insert(v.0); }
                        }
                        Instruction::PtrOffset { ptr, offset } => {
                            if let Operand::Value(v) = ptr { used_values.insert(v.0); }
                            if let Operand::Value(v) = offset { used_values.insert(v.0); }
                        }
                        Instruction::Cast { value: ptr, .. } => {
                            if let Operand::Value(v) = ptr { used_values.insert(v.0); }
                        }
                        Instruction::MakeClosure { env_ptr, .. } => {
                            if let Operand::Value(v) = env_ptr { used_values.insert(v.0); }
                        }
                        Instruction::MakeTraitObject { data_ptr, .. } => {
                            if let Operand::Value(v) = data_ptr { used_values.insert(v.0); }
                        }
                        Instruction::MakeSlice { data_ptr, len } => {
                            if let Operand::Value(v) = data_ptr { used_values.insert(v.0); }
                            if let Operand::Value(v) = len { used_values.insert(v.0); }
                        }
                        _ => {}
                    }
                }
            }
            
            
            let mut pass_changed = false;
            
            // Now remove unused instructions from blocks
            for block in &mut func.blocks {
                let initial_len = block.insts.len();
                block.insts.retain(|inst_id| {
                    let val = &func.values[inst_id.0 as usize];
                    // Keep instructions with side effects or used ones
                    // INVARIANT: mọi Call, BoxNew, BoxFree, Drop đều effectful trừ khi purity analysis chứng minh ngược lại (TRIPWIRE CẢNH BÁO).
                    let has_side_effects = matches!(val.inst, Instruction::Store { .. } | Instruction::CallDirect { .. } | Instruction::CallIndirect { .. } | Instruction::CallClosure { .. } | Instruction::CallVirt { .. } | Instruction::CallIntrinsic { .. } | Instruction::MakeClosure { .. } | Instruction::HeapAlloc | Instruction::BoxNew { .. } | Instruction::BoxFree { .. } | Instruction::Drop { .. } | Instruction::DropVirt { .. } | Instruction::BoundsCheck { .. });
                    has_side_effects || used_values.contains(&inst_id.0) || (inst_id.0 < func.arg_count as u32)
                });
                if block.insts.len() != initial_len {
                    pass_changed = true;
                }
            }
            
            if pass_changed {
                changed = true;
            } else {
                break;
            }
        }
        
        changed
    }
}

impl DeadCodeElimination {
    pub fn new() -> Self {
        Self
    }
}
