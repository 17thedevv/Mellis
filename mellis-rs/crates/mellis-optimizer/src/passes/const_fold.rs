use mellis_mvir::{Function, Instruction, Operand, ValueId};
use crate::pass::Pass;

pub struct ConstantFolding;

impl Pass for ConstantFolding {
    fn name(&self) -> &'static str {
        "ConstantFolding"
    }

    fn run_on_function(&mut self, func: &mut Function) -> bool {
        let mut changed = false;

        for i in 0..func.values.len() {
            let val = &func.values[i];
            let new_inst = match &val.inst {
                Instruction::Add { left, right } => {
                    if let (Some(l), Some(r)) = (Self::resolve_const(func, left), Self::resolve_const(func, right)) {
                        Some(Instruction::Assign(Operand::Number((l + r).to_string())))
                    } else { None }
                }
                Instruction::Sub { left, right } => {
                    if let (Some(l), Some(r)) = (Self::resolve_const(func, left), Self::resolve_const(func, right)) {
                        Some(Instruction::Assign(Operand::Number((l - r).to_string())))
                    } else { None }
                }
                Instruction::Mul { left, right } => {
                    if let (Some(l), Some(r)) = (Self::resolve_const(func, left), Self::resolve_const(func, right)) {
                        Some(Instruction::Assign(Operand::Number((l * r).to_string())))
                    } else { None }
                }
                Instruction::Eq { left, right } => {
                    if let (Some(l), Some(r)) = (Self::resolve_const(func, left), Self::resolve_const(func, right)) {
                        Some(Instruction::Assign(Operand::Boolean(l == r)))
                    } else { None }
                }
                _ => None,
            };

            if let Some(inst) = new_inst {
                func.values[i].inst = inst;
                changed = true;
            }
        }

        changed
    }
}

impl ConstantFolding {
    pub fn new() -> Self {
        Self
    }
    
    // Attempt to resolve an operand to a constant integer
    fn resolve_const(func: &Function, op: &Operand) -> Option<i64> {
        match op {
            Operand::Number(n) => n.parse::<i64>().ok(),
            Operand::Value(val_id) => {
                let def = func.value(*val_id);
                match &def.inst {
                    Instruction::Assign(Operand::Number(n)) => n.parse::<i64>().ok(),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
