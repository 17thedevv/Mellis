use mellis_mvir::{Module, Function, Terminator, Operand, Instruction};
use std::collections::HashSet;

pub fn verify_module(module: &Module) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for func in &module.functions {
        if let Err(mut e) = verify_function(func) {
            errors.append(&mut e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn verify_function(func: &Function) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    
    // Collect all valid block labels
    let block_labels: HashSet<&str> = func.blocks.iter().map(|b| b.label.name.as_str()).collect();

    let check_operand = |op: &Operand, errors: &mut Vec<String>, context: &str| {
        match op {
            Operand::Value(val_id) => {
                if val_id.0 as usize >= func.values.len() {
                    errors.push(format!("{}: Invalid ValueId {}", context, val_id.0));
                }
            }
            Operand::Block(blk_id) => {
                if blk_id.0 as usize >= func.blocks.len() {
                    errors.push(format!("{}: Invalid BlockId {}", context, blk_id.0));
                }
            }
            _ => {}
        }
    };

    // Check blocks
    for (i, block) in func.blocks.iter().enumerate() {
        // Check instructions
        for inst_id in &block.insts {
            if inst_id.0 as usize >= func.values.len() {
                errors.push(format!("Block '{}' contains invalid ValueId {}", block.label.name, inst_id.0));
            }
        }
        
        // Check terminator
        if let Some(term) = &block.terminator {
            match term {
                Terminator::Br { target } => {
                    if !block_labels.contains(target.name.as_str()) {
                        errors.push(format!("Block '{}' branches to unknown label '{}'", block.label.name, target.name));
                    }
                }
                Terminator::CondBr { condition, true_target, false_target } => {
                    check_operand(condition, &mut errors, &format!("Block '{}' CondBr condition", block.label.name));
                    if !block_labels.contains(true_target.name.as_str()) {
                        errors.push(format!("Block '{}' cond branches to unknown true label '{}'", block.label.name, true_target.name));
                    }
                    if !block_labels.contains(false_target.name.as_str()) {
                        errors.push(format!("Block '{}' cond branches to unknown false label '{}'", block.label.name, false_target.name));
                    }
                }
                Terminator::Ret { value } => {
                    if let Some(v) = value {
                        check_operand(v, &mut errors, &format!("Block '{}' Ret", block.label.name));
                    }
                }
                Terminator::Unreachable => {}
            }
        } else {
            errors.push(format!("Block '{}' is missing a terminator", block.label.name));
        }
    }
    
    // Check operands in values
    for (i, value) in func.values.iter().enumerate() {
        let ctx = format!("Value {}", i);
        match &value.inst {
            Instruction::Alloca => {}
            Instruction::Assign(val) => {
                check_operand(val, &mut errors, &ctx);
            }
            Instruction::Store { ptr, value: val } => {
                check_operand(ptr, &mut errors, &ctx);
                check_operand(val, &mut errors, &ctx);
            }
            Instruction::Load { ptr } => {
                check_operand(ptr, &mut errors, &ctx);
            }
            Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } | Instruction::Eq { left, right } => {
                check_operand(left, &mut errors, &ctx);
                check_operand(right, &mut errors, &ctx);
            }
            Instruction::Call { callee, args } => {
                check_operand(callee, &mut errors, &ctx);
                for arg in args {
                    check_operand(arg, &mut errors, &ctx);
                }
            }
            Instruction::BoundsCheck { index, len } => {
                check_operand(index, &mut errors, &ctx);
                check_operand(len, &mut errors, &ctx);
            }
            Instruction::Borrow { base, .. } => {
                check_operand(base, &mut errors, &ctx);
            }
            Instruction::Variant { args, .. } => {
                for arg in args {
                    check_operand(arg, &mut errors, &ctx);
                }
            }
            Instruction::Tag { value } => {
                check_operand(value, &mut errors, &ctx);
            }
            Instruction::Extract { value, .. } => {
                check_operand(value, &mut errors, &ctx);
            }
        }
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
