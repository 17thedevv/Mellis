use std::collections::HashMap;
use mellis_common::Diagnostic;
use mellis_mvir::{Function, Instruction, Terminator, Operand};
use crate::dataflow::DataflowAnalysis;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveState {
    Uninitialized,
    Initialized,
    Moved,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveStateData {
    pub locals: HashMap<String, MoveState>,
}

pub struct MoveAnalyzer {
    pub diagnostics: Vec<Diagnostic>,
}

impl MoveAnalyzer {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }
    
    fn check_operand(&mut self, op: &Operand, state: &MoveStateData) {
        if let Operand::Value(val) = op {
            let name = format!("%v{}", val.0);
            let loc_state = state.locals.get(&name).unwrap_or(&MoveState::Initialized);
            if loc_state == &MoveState::Moved {
                self.diagnostics.push(Diagnostic::error(
                    format!("Use of moved value '{}'", name),
                ));
            } else if loc_state == &MoveState::Uninitialized {
                self.diagnostics.push(Diagnostic::error(
                    format!("Use of uninitialized value '{}'", name),
                ));
            }
        }
    }

    fn mark_moved(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            state.locals.insert(format!("%v{}", val.0), MoveState::Moved);
        }
    }
}

impl DataflowAnalysis<MoveStateData> for MoveAnalyzer {
    fn transfer_instruction(&mut self, inst: &Instruction, state: &mut MoveStateData) {
        match inst {
            Instruction::Alloca => {
            }
            Instruction::Assign(op) => {
                self.check_operand(op, state);
                self.mark_moved(op, state);
            }
            Instruction::Store { ptr, value } => {
                self.check_operand(value, state);
                self.mark_moved(value, state); // Value is moved into ptr
                
                if let Operand::Value(dest_val) = ptr {
                    state.locals.insert(format!("%v{}", dest_val.0), MoveState::Initialized);
                }
            }
            Instruction::Load { ptr, .. } => {
                self.check_operand(ptr, state);
            }
            Instruction::Call { args, callee, .. } => {
                self.check_operand(callee, state);
                for arg in args {
                    self.check_operand(arg, state);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::BoundsCheck { index, len } => {
                self.check_operand(index, state);
                self.check_operand(len, state);
            }
            Instruction::Borrow { base, .. } => {
                self.check_operand(base, state);
            }
            Instruction::Add { left, right, .. } 
            | Instruction::Sub { left, right, .. }
            | Instruction::Mul { left, right, .. }
            | Instruction::Eq { left, right, .. } => {
                self.check_operand(left, state);
                self.check_operand(right, state);
            }
            Instruction::Variant { args, .. } => {
                for arg in args {
                    self.check_operand(arg, state);
                    self.mark_moved(arg, state);
                }
            }
            Instruction::Tag { value } | Instruction::Extract { value, .. } => {
                self.check_operand(value, state);
            }
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut MoveStateData) {
        match term {
            Terminator::Ret { value } => {
                if let Some(val) = value {
                    self.check_operand(val, state);
                    self.mark_moved(val, state);
                }
            }
            Terminator::CondBr { condition, .. } => {
                self.check_operand(condition, state);
            }
            _ => {}
        }
    }

    fn merge(&mut self, dest: &mut MoveStateData, src: &MoveStateData) -> bool {
        let mut changed = false;
        for (k, v) in &src.locals {
            let dest_val = dest.locals.entry(k.clone()).or_insert(MoveState::Uninitialized);
            if dest_val != v {
                // If one path moves, the joined path is moved
                if *v == MoveState::Moved || *dest_val == MoveState::Moved {
                    *dest_val = MoveState::Moved;
                    changed = true;
                } else if *v == MoveState::Initialized && *dest_val == MoveState::Uninitialized {
                    *dest_val = MoveState::Initialized;
                    changed = true;
                }
            }
        }
        changed
    }

    fn init_entry_state(&mut self, func: &Function, state: &mut MoveStateData) {
        for (idx, val_data) in func.values.iter().enumerate() {
            if matches!(val_data.inst, Instruction::Alloca) {
                state.locals.insert(format!("%v{}", idx), MoveState::Uninitialized);
            }
        }
    }
}
