use std::collections::HashSet;
use mellis_common::Diagnostic;
use mellis_mvir::{Function, Instruction, Terminator, Operand};
use crate::dataflow::DataflowAnalysis;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Loan {
    pub id: usize,
    pub place: Operand,
    pub is_rw: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorrowStateData {
    pub active_loans: HashSet<Loan>,
}

pub struct BorrowAnalyzer {
    pub diagnostics: Vec<Diagnostic>,
    next_loan_id: usize,
}

impl BorrowAnalyzer {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            next_loan_id: 1,
        }
    }

    fn check_access(&mut self, place: &Operand, is_write: bool, state: &BorrowStateData) {
        let place_name = print_operand_name(place);
        for loan in &state.active_loans {
            if print_operand_name(&loan.place) == place_name {
                if loan.is_rw {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Cannot access '{}' because it is borrowed as &rw", print_operand_name(place))
                    ));
                } else if is_write {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Cannot write to '{}' because it is borrowed as &", print_operand_name(place))
                    ));
                }
            }
        }
    }

    fn issue_loan(&mut self, place: &Operand, is_rw: bool, state: &mut BorrowStateData) {
        let place_name = print_operand_name(place);
        // Check if we can issue the loan
        for loan in &state.active_loans {
            if print_operand_name(&loan.place) == place_name {
                if loan.is_rw {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Cannot borrow '{}' as {} because it is already borrowed as &rw", 
                                print_operand_name(place), if is_rw { "&rw" } else { "&" })
                    ));
                } else if is_rw {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Cannot borrow '{}' as &rw because it is already borrowed as &", print_operand_name(place))
                    ));
                }
            }
        }

        state.active_loans.insert(Loan {
            id: self.next_loan_id,
            place: place.clone(),
            is_rw,
        });
        self.next_loan_id += 1;
    }
}

fn print_operand_name(op: &Operand) -> String {
    match op {
        Operand::Value(val) => format!("%v{}", val.0),
        Operand::Global(glb) => glb.name.clone(),
        Operand::Block(blk) => format!("%block_{}", blk.0),
        Operand::Number(n) => n.clone(),
        Operand::Boolean(b) => b.to_string(),
    }
}

impl DataflowAnalysis<BorrowStateData> for BorrowAnalyzer {
    fn transfer_instruction(&mut self, inst: &Instruction, state: &mut BorrowStateData) {
        match inst {
            Instruction::Store { ptr, value } => {
                self.check_access(value, false, state);
                self.check_access(ptr, true, state);
            }
            Instruction::Load { ptr, .. } => {
                self.check_access(ptr, false, state);
            }
            Instruction::Call { args, callee, .. } => {
                self.check_access(callee, false, state);
                for arg in args {
                    self.check_access(arg, false, state);
                }
            }
            Instruction::Add { left, right, .. } 
            | Instruction::Sub { left, right, .. }
            | Instruction::Mul { left, right, .. } => {
                self.check_access(left, false, state);
                self.check_access(right, false, state);
            }
            Instruction::Borrow { is_rw, base, .. } => {
                self.issue_loan(base, *is_rw, state);
            }
            _ => {}
        }
    }

    fn transfer_terminator(&mut self, term: &Terminator, state: &mut BorrowStateData) {
        match term {
            Terminator::Ret { value } => {
                if let Some(val) = value {
                    self.check_access(val, false, state);
                }
            }
            Terminator::CondBr { condition, .. } => {
                self.check_access(condition, false, state);
            }
            _ => {}
        }
    }

    fn merge(&mut self, dest: &mut BorrowStateData, src: &BorrowStateData) -> bool {
        let mut changed = false;
        for loan in &src.active_loans {
            if dest.active_loans.insert(loan.clone()) {
                changed = true;
            }
        }
        changed
    }

    fn init_entry_state(&mut self, _func: &Function, _state: &mut BorrowStateData) {}
}
