pub mod dataflow;
pub mod place;
pub mod move_analysis;
pub mod borrow_analysis;

use mellis_common::Diagnostic;
use mellis_mvir::Function;
use mellis_semantic::SemanticContext;
use crate::dataflow::DataflowEngine;
use crate::move_analysis::MoveAnalyzer;

pub fn borrow_check_function(func: &Function, _ctx: &SemanticContext) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Run Move Analysis
    let mut move_analyzer = MoveAnalyzer::new();
    let _states = DataflowEngine::run_forward(func, &mut move_analyzer);
    diagnostics.extend(move_analyzer.diagnostics);

    // 2. Run Borrow Analysis (Loans)
    let mut borrow_analyzer = crate::borrow_analysis::BorrowAnalyzer::new();
    let _states = DataflowEngine::run_forward(func, &mut borrow_analyzer);
    diagnostics.extend(borrow_analyzer.diagnostics);

    diagnostics
}
