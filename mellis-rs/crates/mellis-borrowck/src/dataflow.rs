use std::collections::HashMap;
use mellis_mvir::{Function, BasicBlock};

pub trait DataflowAnalysis<State> {
    fn transfer_instruction(&mut self, inst: &mellis_mvir::Instruction, state: &mut State);
    fn transfer_terminator(&mut self, term: &mellis_mvir::Terminator, state: &mut State);
    fn merge(&mut self, dest: &mut State, src: &State) -> bool;
    fn init_entry_state(&mut self, func: &Function, state: &mut State);
}

pub struct DataflowEngine;

impl DataflowEngine {
    pub fn run_forward<State: Clone + Default + PartialEq, A: DataflowAnalysis<State>>(
        func: &Function,
        analysis: &mut A,
    ) -> HashMap<String, State> {
        let mut block_states: HashMap<String, State> = HashMap::new();
        
        // Initialize all blocks
        for block in &func.blocks {
            block_states.insert(block.label.name.clone(), State::default());
        }
        
        if let Some(entry) = func.blocks.first() {
            let mut entry_state = State::default();
            analysis.init_entry_state(func, &mut entry_state);
            block_states.insert(entry.label.name.clone(), entry_state);
        }
        
        let mut changed = true;
        while changed {
            changed = false;
            
            for block in &func.blocks {
                let mut current_state = block_states[&block.label.name].clone();
                
                for &val_id in &block.insts {
                    let val_data = func.value(val_id);
                    analysis.transfer_instruction(&val_data.inst, &mut current_state);
                }
                
                if let Some(term) = &block.terminator {
                    analysis.transfer_terminator(term, &mut current_state);
                    
                    // Propagate to successors
                    let successors = match term {
                        mellis_mvir::Terminator::Br { target } => vec![target.name.clone()],
                        mellis_mvir::Terminator::CondBr { true_target, false_target, .. } => {
                            vec![true_target.name.clone(), false_target.name.clone()]
                        },
                        _ => vec![],
                    };
                    
                    for succ in successors {
                        let dest_state = block_states.get_mut(&succ).unwrap();
                        if analysis.merge(dest_state, &current_state) {
                            changed = true;
                        }
                    }
                }
            }
        }
        
        block_states
    }
}
