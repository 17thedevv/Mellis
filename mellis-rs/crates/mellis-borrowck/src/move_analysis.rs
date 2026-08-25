use crate::dataflow::DataflowAnalysis;
use mellis_common::Diagnostic;
use mellis_mvir::{Function, Instruction, Operand, Terminator};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveState {
    Uninitialized,
    Live,
    Moved,
    Dropped,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoveStateData {
    pub locals: HashMap<String, MoveState>,
}

pub struct MoveAnalyzer {
    pub diagnostics: Vec<Diagnostic>,
    pub emit_diagnostics: bool,
    pub load_origins: std::collections::HashMap<mellis_mvir::ValueId, mellis_mvir::ValueId>,
}

impl MoveAnalyzer {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            emit_diagnostics: false,
            load_origins: std::collections::HashMap::new(),
        }
    }

    fn resolve_val(&self, val: mellis_mvir::ValueId) -> mellis_mvir::ValueId {
        let mut curr = val;
        while let Some(&origin) = self.load_origins.get(&curr) {
            if curr == origin { break; }
            curr = origin;
        }
        curr
    }

    fn check_operand(&mut self, op: &Operand, state: &MoveStateData) {
        if !self.emit_diagnostics { return; }
        if let Operand::Value(val) = op {
            let actual_val = self.resolve_val(*val);
            let name = format!("%v{}", actual_val.0);
            let loc_state = state.locals.get(&name).unwrap_or(&MoveState::Live);
            if loc_state == &MoveState::Moved {
                let msg = format!("Use of moved value '{}'", name);
                if !self.diagnostics.iter().any(|d| d.message == msg) {
                    self.diagnostics.push(Diagnostic::error(msg));
                }
            } else if loc_state == &MoveState::Dropped {
                let msg = format!("Use of dropped value '{}'", name);
                if !self.diagnostics.iter().any(|d| d.message == msg) {
                    self.diagnostics.push(Diagnostic::error(msg));
                }
            } else if loc_state == &MoveState::Uninitialized {
                let msg = format!("Use of uninitialized value '{}'", name);
                if !self.diagnostics.iter().any(|d| d.message == msg) {
                    self.diagnostics.push(Diagnostic::error(msg));
                }
            }
        }
    }

    fn mark_moved(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            let actual_val = self.resolve_val(*val);
            state
                .locals
                .insert(format!("%v{}", actual_val.0), MoveState::Moved);
        }
    }

    fn mark_dropped(&mut self, op: &Operand, state: &mut MoveStateData) {
        if let Operand::Value(val) = op {
            let actual_val = self.resolve_val(*val);
            let name = format!("%v{}", actual_val.0);
            let loc_state = state.locals.get(&name).unwrap_or(&MoveState::Live).clone();
            // If it is already Moved, leave it Moved. 
            // If it is Live, mark it as Dropped.
            if loc_state != MoveState::Moved {
                state.locals.insert(name, MoveState::Dropped);
            }
        }
    }
}

impl DataflowAnalysis<MoveStateData> for MoveAnalyzer {
    fn transfer_instruction(
        &mut self,
        _val_id: mellis_mvir::ValueId,
        inst: &Instruction,
        state: &mut MoveStateData,
    ) {
        if !matches!(inst, Instruction::Alloca) {
            state.locals.insert(format!("%v{}", _val_id.0), MoveState::Live);
        }
        
        match inst {
            Instruction::Alloca => {}
            Instruction::Assign(op) => {
                self.check_operand(op, state);
                self.mark_moved(op, state);
            }
            Instruction::Store { ptr, value } => {
                self.check_operand(value, state);
                self.mark_moved(value, state); // Value is moved into ptr

                if let Operand::Value(dest_val) = ptr {
                    state
                        .locals
                        .insert(format!("%v{}", dest_val.0), MoveState::Live);
                }
            }
            Instruction::Load { ptr, .. } => {
                self.check_operand(ptr, state);
                if let Operand::Value(ptr_val) = ptr {
                    self.load_origins.insert(_val_id, *ptr_val);
                }
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
            Instruction::Drop { value } => {
                self.mark_dropped(value, state);
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
            let entry = dest.locals.entry(k.clone()).or_insert(MoveState::Live);
            let new_state = match (entry.clone(), v.clone()) {
                (MoveState::Uninitialized, _) | (_, MoveState::Uninitialized) => MoveState::Uninitialized,
                (MoveState::Dropped, _) | (_, MoveState::Dropped) => MoveState::Dropped,
                (MoveState::Moved, _) | (_, MoveState::Moved) => MoveState::Moved,
                _ => MoveState::Live,
            };
            if *entry != new_state {
                *entry = new_state;
                changed = true;
            }
        }
        changed
    }

    fn init_entry_state(&mut self, func: &Function, state: &mut MoveStateData) {
        let mut alloca_count = 0;
        for (idx, val_data) in func.values.iter().enumerate() {
            if matches!(val_data.inst, Instruction::Alloca) {
                let init_state = if alloca_count < func.arg_count {
                    MoveState::Live
                } else {
                    MoveState::Uninitialized
                };
                
                state
                    .locals
                    .insert(format!("%v{}", idx), init_state);
                    
                alloca_count += 1;
            }
        }
    }
}
