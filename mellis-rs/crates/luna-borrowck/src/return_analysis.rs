use luna_common::Diagnostic;
use luna_mvir::{Function, Terminator};
use std::collections::{HashMap, HashSet};

pub fn analyze_returns(func: &Function) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    if func.blocks.is_empty() {
        return diagnostics;
    }
    
    // Map block label names to their index in the blocks array
    let mut label_to_idx = HashMap::new();
    for (i, block) in func.blocks.iter().enumerate() {
        label_to_idx.insert(block.label.name.clone(), i);
    }
    
    let mut visited = HashSet::new();
    let mut stack = vec![0]; // entry block is usually 0
    
    while let Some(block_idx) = stack.pop() {
        if !visited.insert(block_idx) {
            continue;
        }
        
        if block_idx >= func.blocks.len() { continue; }
        let block = &func.blocks[block_idx];
        if let Some(terminator) = &block.terminator {
            match terminator {
                Terminator::Br { target } => {
                    if let Some(&idx) = label_to_idx.get(&target.name) {
                        stack.push(idx);
                    }
                }
                Terminator::CondBr { true_target, false_target, .. } => {
                    if let Some(&idx) = label_to_idx.get(&true_target.name) {
                        stack.push(idx);
                    }
                    if let Some(&idx) = label_to_idx.get(&false_target.name) {
                        stack.push(idx);
                    }
                }
                Terminator::MissingReturn => {
                    let diag = Diagnostic::error("Function expects a return value, but control flow reaches the end without returning.".to_string());
                    diagnostics.push(diag);
                }
                Terminator::Ret { .. } | Terminator::Unreachable => {}
            }
        }
    }
    
    diagnostics
}
