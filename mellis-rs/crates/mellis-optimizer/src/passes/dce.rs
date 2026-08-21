use mellis_mvir::{Function, Instruction, Terminator, Operand};
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
                        Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } => {
                            if let Operand::Value(v) = left { used_values.insert(v.0); }
                            if let Operand::Value(v) = right { used_values.insert(v.0); }
                        }
                        Instruction::Call { callee, args } => {
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
                    let has_side_effects = matches!(val.inst, Instruction::Store { .. } | Instruction::Call { .. });
                    has_side_effects || used_values.contains(&inst_id.0)
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
