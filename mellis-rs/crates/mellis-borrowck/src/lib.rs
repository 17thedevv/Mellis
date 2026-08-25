pub mod borrow_analysis;
pub mod dataflow;
pub mod effect;
pub mod effect_inference;
pub mod interprocedural;
pub mod move_analysis;
pub mod place;

use crate::dataflow::{DataflowEngine, DataflowAnalysis};
use crate::move_analysis::MoveAnalyzer;
use mellis_common::Diagnostic;
use mellis_mvir::{Function, GlobalId};
use mellis_semantic::SemanticContext;
use std::collections::HashMap;

pub fn borrow_check_function(
    func: &Function, 
    _ctx: &SemanticContext, 
    summaries: &HashMap<GlobalId, crate::effect::CallEffectSummary>
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Run Move Analysis
    let mut move_analyzer = MoveAnalyzer::new();
    let move_states = DataflowEngine::run_forward(func, &mut move_analyzer);
    move_analyzer.emit_diagnostics = true;
    for block in &func.blocks {
        let mut current_state = move_states.get(&block.label.name).cloned().unwrap_or_default();
        for &val_id in &block.insts {
            let val_data = func.value(val_id);
            move_analyzer.transfer_instruction(val_id, &val_data.inst, &mut current_state);
        }
    }
    diagnostics.extend(move_analyzer.diagnostics);

    // 2. Run Borrow Analysis (Loans)
    let mut borrow_diagnostics = crate::borrow_analysis::BorrowAnalyzer::analyze(func, Some(summaries), Some(_ctx));
    diagnostics.append(&mut borrow_diagnostics);

    diagnostics
}
