use crate::effect::{CallEffectSummary, ReturnEffect};
use crate::effect_inference::EffectInference;
use mellis_mvir::{Function, GlobalId, Module, Operand, Instruction};
use std::collections::{HashMap, HashSet};

pub struct InterproceduralContext {
    pub summaries: HashMap<GlobalId, CallEffectSummary>,
}

impl InterproceduralContext {
    pub fn new() -> Self {
        Self {
            summaries: HashMap::new(),
        }
    }

    /// Computes the fixed-point summaries for all functions in the module.
    pub fn compute_summaries(&mut self, module: &Module) {
        let mut worklist: HashSet<GlobalId> = module.functions.iter().map(|f| f.name.clone()).collect();
        let mut f_map: HashMap<GlobalId, &Function> = HashMap::new();
        let mut callers: HashMap<GlobalId, Vec<GlobalId>> = HashMap::new();
        
        for f in &module.functions {
            f_map.insert(f.name.clone(), f);
            
            // Build dependency graph (reverse call graph) to know who to add to worklist
            // when a callee's summary changes.
            for block in &f.blocks {
                for &val_id in &block.insts {
                    let val_data = &f.values[val_id.0 as usize];
                    if let Instruction::CallDirect { callee, .. } = &val_data.inst {
                        callers.entry(callee.clone()).or_default().push(f.name.clone());
                    }
                }
            }
        }
        
        // Initialize default summaries
        for f in &module.functions {
            if f.is_extern {
                continue;
            }
            // Find argument count by looking at values up to the first non-Alloca?
            // Wait, we need to know how many arguments the function has.
            // In MVIR, arguments are represented as Alloca instructions at the beginning of the entry block.
            // Let's assume `count_arguments` for now.
            let arg_count = count_arguments(f);
            self.summaries.insert(f.name.clone(), CallEffectSummary::default_for_args(arg_count));
        }

        // Fixed-point iteration
        let mut changed = true;
        while changed {
            changed = false;
            let current_worklist: Vec<GlobalId> = worklist.drain().collect();
            
            for gid in current_worklist {
                if let Some(&func) = f_map.get(&gid) {
                    if func.is_extern { continue; }
                    let arg_count = count_arguments(func);
                    let mut arg_values = vec![];
                    for i in 0..arg_count {
                        arg_values.push(mellis_mvir::ValueId(i as u32));
                    }
                    
                    let old_summary = self.summaries.get(&gid).cloned().unwrap();
                    let new_summary = EffectInference::infer(func, arg_values, Some(&self.summaries));
                    
                    if old_summary != new_summary {
                        self.summaries.insert(func.name.clone(), new_summary.clone());
                        changed = true;
                        
                        // Add all callers to worklist
                        if let Some(callers_list) = callers.get(&gid) {
                            for caller in callers_list {
                                worklist.insert(caller.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A simple heuristic to count arguments by assuming the first N values in the entry block
/// that are `Alloca` are the arguments. This depends on how MVIR is generated.
fn count_arguments(func: &Function) -> usize {
    func.arg_count
}
