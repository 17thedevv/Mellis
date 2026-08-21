use crate::mvir::*;

pub fn print_module(module: &Module) -> String {
    let mut out = String::new();
    for func in &module.functions {
        out.push_str(&format!("fn {}() {{\n", func.name.name));
        for block in &func.blocks {
            out.push_str(&format!("{}:\n", block.label.name));
            for &val_id in &block.insts {
                let val_data = func.value(val_id);
                out.push_str(&format!("  %v{} = {}\n", val_id.0, print_instruction(&val_data.inst)));
            }
            if let Some(term) = &block.terminator {
                out.push_str(&format!("  {}\n", print_terminator(term)));
            } else {
                out.push_str("  <missing terminator>\n");
            }
        }
        out.push_str("}\n\n");
    }
    out
}

fn print_instruction(inst: &Instruction) -> String {
    match inst {
        Instruction::Alloca => {
            "alloca".to_string()
        }
        Instruction::Assign(val) => {
            format!("assign {}", print_operand(val))
        }
        Instruction::Store { ptr, value } => {
            format!("store {} -> {}", print_operand(value), print_operand(ptr))
        }
        Instruction::Load { ptr } => {
            format!("load {}", print_operand(ptr))
        }
        Instruction::Add { left, right } => {
            format!("add {}, {}", print_operand(left), print_operand(right))
        }
        Instruction::Sub { left, right } => {
            format!("sub {}, {}", print_operand(left), print_operand(right))
        }
        Instruction::Mul { left, right } => {
            format!("mul {}, {}", print_operand(left), print_operand(right))
        }
        Instruction::Eq { left, right } => {
            format!("eq {}, {}", print_operand(left), print_operand(right))
        }
        Instruction::Call { callee, args } => {
            let mut arg_strs = Vec::new();
            for arg in args {
                arg_strs.push(print_operand(arg));
            }
            format!("call {}({})", print_operand(callee), arg_strs.join(", "))
        }
        Instruction::BoundsCheck { index, len } => {
            format!("bounds_check {} < {}", print_operand(index), print_operand(len))
        }
        Instruction::Borrow { is_rw, base } => {
            let kw = if *is_rw { "mut " } else { "" };
            format!("borrow &{}{}", kw, print_operand(base))
        }
        Instruction::Variant { enum_ty, variant_idx, args } => {
            let mut arg_strs = Vec::new();
            for arg in args {
                arg_strs.push(print_operand(arg));
            }
            format!("variant {}::{}({})", enum_ty.0, variant_idx, arg_strs.join(", "))
        }
        Instruction::Tag { value } => {
            format!("tag {}", print_operand(value))
        }
        Instruction::Extract { value, variant_idx, field_idx } => {
            format!("extract {}.{}.{}", print_operand(value), variant_idx, field_idx)
        }
    }
}

fn print_terminator(term: &Terminator) -> String {
    match term {
        Terminator::Ret { value } => {
            if let Some(val) = value {
                format!("ret {}", print_operand(val))
            } else {
                "ret void".to_string()
            }
        }
        Terminator::Br { target } => {
            format!("br {}", target.name)
        }
        Terminator::CondBr { condition, true_target, false_target } => {
            format!("condbr {}, {}, {}", print_operand(condition), true_target.name, false_target.name)
        }
        Terminator::Unreachable => {
            "unreachable".to_string()
        }
    }
}

fn print_operand(op: &Operand) -> String {
    match op {
        Operand::Value(val_id) => format!("%v{}", val_id.0),
        Operand::Global(glb) => glb.name.clone(),
        Operand::Block(blk_id) => format!("block_{}", blk_id.0),
        Operand::Number(n) => n.clone(),
        Operand::Boolean(b) => b.to_string(),
    }
}
