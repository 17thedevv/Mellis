use crate::dataflow::{DataflowAnalysis, DataflowEngine};
use crate::move_analysis::{MoveAnalyzer, MoveState, MoveStateData};
use crate::place::Place;
use luna_mvir::{Function, Instruction, ValueId};
use luna_semantic::SemanticContext;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SuspensionState {
    pub initialized_places: HashSet<Place>,
    pub moved_places: HashSet<Place>,
    pub live_places: HashSet<Place>,
}

#[derive(Debug, Clone)]
pub struct AsyncStateMap {
    pub initial_state: SuspensionState,
    pub await_states: HashMap<ValueId, SuspensionState>,
}

fn extract_live_places(current_state: &MoveStateData) -> (HashSet<Place>, HashSet<Place>, HashSet<Place>) {
    let mut initialized = HashSet::new();
    let mut moved = HashSet::new();
    let mut live_raw = HashSet::new();

    for (place, state) in &current_state.places {
        match state {
            MoveState::Live => {
                live_raw.insert(place.clone());
                initialized.insert(place.clone());
            }
            MoveState::PartialMoved => {
                initialized.insert(place.clone());
            }
            MoveState::ConditionallyMoved => {
                live_raw.insert(place.clone());
                initialized.insert(place.clone());
            }
            MoveState::Moved | MoveState::Dropped => {
                moved.insert(place.clone());
            }
            MoveState::Uninitialized => {}
        }
    }

    // Filter out descendant places if their ancestor is already Live (to avoid double drops).
    let mut live = HashSet::new();
    for p in &live_raw {
        let has_live_ancestor = live_raw.iter().any(|other| other != p && p.is_descendant_of(other));
        if !has_live_ancestor {
            live.insert(p.clone());
        }
    }

    (initialized, moved, live)
}

pub fn compute_suspension_states(
    func: &Function,
    semantic_ctx: Option<&SemanticContext>,
) -> AsyncStateMap {
    let mut move_analyzer = MoveAnalyzer::new(func, semantic_ctx, None);
    move_analyzer.emit_diagnostics = false;

    let block_states = DataflowEngine::run_forward(func, &mut move_analyzer);
    let mut await_states = HashMap::new();
    let mut initial_state_opt = None;

    if let Some(entry_block) = func.blocks.first() {
        let mut current_state = block_states.get(&entry_block.label.name).cloned().unwrap_or_default();

        // 1. Compute initial state (State 0)
        for val_id in &entry_block.insts {
            let val_data = &func.values[val_id.0 as usize];
            if !matches!(val_data.inst, Instruction::Alloca) {
                break;
            }
            move_analyzer.transfer_instruction(*val_id, &val_data.inst, &mut current_state);
        }

        let (init_initialized, init_moved, init_live) = extract_live_places(&current_state);
        initial_state_opt = Some(SuspensionState {
            initialized_places: init_initialized,
            moved_places: init_moved,
            live_places: init_live,
        });

        // 2. Compute await states
        for block in &func.blocks {
            let mut current_state = block_states.get(&block.label.name).cloned().unwrap_or_default();
            for val_id in &block.insts {
                let val_data = &func.values[val_id.0 as usize];
                if matches!(val_data.inst, Instruction::Await { .. }) {
                    let (initialized, moved, live) = extract_live_places(&current_state);

                    await_states.insert(
                        *val_id,
                        SuspensionState {
                            initialized_places: initialized,
                            moved_places: moved,
                            live_places: live,
                        },
                    );
                }

                // Advance state
                move_analyzer.transfer_instruction(*val_id, &val_data.inst, &mut current_state);
            }
        }
    }

    let default_init = SuspensionState {
        initialized_places: HashSet::new(),
        moved_places: HashSet::new(),
        live_places: HashSet::new(),
    };

    AsyncStateMap {
        initial_state: initial_state_opt.unwrap_or(default_init),
        await_states,
    }
}
