use mellis_mvir::{BasicBlock, Function};
use std::collections::HashMap;

pub trait DataflowAnalysis<State> {
    fn transfer_instruction(
        &mut self,
        val_id: mellis_mvir::ValueId,
        inst: &mellis_mvir::Instruction,
        state: &mut State,
    );
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
        let (states, _) = Self::run_forward_with_stats(func, analysis);
        states
    }

    pub fn run_forward_with_stats<State: Clone + Default + PartialEq, A: DataflowAnalysis<State>>(
        func: &Function,
        analysis: &mut A,
    ) -> (HashMap<String, State>, usize) {
        let mut block_states: HashMap<String, State> = HashMap::new();

        // Initialize all blocks
        for block in &func.blocks {
            block_states.insert(block.label.name.clone(), State::default());
        }

        Self::run_forward_incremental(func, analysis, block_states)
    }

    pub fn run_forward_incremental<State: Clone + Default + PartialEq, A: DataflowAnalysis<State>>(
        func: &Function,
        analysis: &mut A,
        mut block_states: HashMap<String, State>,
    ) -> (HashMap<String, State>, usize) {
        // Ensure all blocks in func exist in block_states
        for block in &func.blocks {
            block_states.entry(block.label.name.clone()).or_insert_with(State::default);
        }

        if let Some(entry) = func.blocks.first() {
            let mut entry_state = State::default();
            analysis.init_entry_state(func, &mut entry_state);
            // Wait, what if this is incremental and the entry state hasn't changed?
            // Actually, we must guarantee the entry block starts with init_entry_state.
            // But we should merge it in case it's incremental and we don't want to lose info.
            // Wait, for forward analysis, entry state is absolute.
            block_states.insert(entry.label.name.clone(), entry_state);
        }

        let mut changed = true;
        let mut iterations = 0;
        
        while changed {
            changed = false;
            iterations += 1;

            for block in &func.blocks {
                let mut current_state = block_states[&block.label.name].clone();

                for &val_id in &block.insts {
                    let val_data = func.value(val_id);
                    analysis.transfer_instruction(val_id, &val_data.inst, &mut current_state);
                }

                if let Some(term) = &block.terminator {
                    analysis.transfer_terminator(term, &mut current_state);

                    // Propagate to successors
                    let successors = match term {
                        mellis_mvir::Terminator::Br { target } => vec![target.name.clone()],
                        mellis_mvir::Terminator::CondBr {
                            true_target,
                            false_target,
                            ..
                        } => {
                            vec![true_target.name.clone(), false_target.name.clone()]
                        }
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
        
        (block_states, iterations)
    }
    pub fn run_backward<State: Clone + Default + PartialEq, A: DataflowAnalysis<State>>(
        func: &Function,
        analysis: &mut A,
    ) -> HashMap<String, State> {
        let mut block_states: HashMap<String, State> = HashMap::new();

        // Initialize all blocks
        for block in &func.blocks {
            block_states.insert(block.label.name.clone(), State::default());
        }

        if let Some(exit) = func.blocks.last() {
            let mut exit_state = State::default();
            analysis.init_entry_state(func, &mut exit_state);
            block_states.insert(exit.label.name.clone(), exit_state);
        }

        // Precompute predecessors
        let mut predecessors: HashMap<String, Vec<String>> = HashMap::new();
        for block in &func.blocks {
            if let Some(term) = &block.terminator {
                let successors = match term {
                    mellis_mvir::Terminator::Br { target } => vec![target.name.clone()],
                    mellis_mvir::Terminator::CondBr {
                        true_target,
                        false_target,
                        ..
                    } => {
                        vec![true_target.name.clone(), false_target.name.clone()]
                    }
                    _ => vec![],
                };
                for succ in successors {
                    predecessors.entry(succ).or_default().push(block.label.name.clone());
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;

            // In backward dataflow, we usually iterate blocks in reverse postorder, but standard reverse iteration is fine for a fixed-point.
            for block in func.blocks.iter().rev() {
                let mut current_state = block_states[&block.label.name].clone();

                if let Some(term) = &block.terminator {
                    analysis.transfer_terminator(term, &mut current_state);
                }

                for &val_id in block.insts.iter().rev() {
                    let val_data = func.value(val_id);
                    analysis.transfer_instruction(val_id, &val_data.inst, &mut current_state);
                }

                // Propagate to predecessors
                if let Some(preds) = predecessors.get(&block.label.name) {
                    for pred in preds {
                        let dest_state = block_states.get_mut(pred).unwrap();
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
