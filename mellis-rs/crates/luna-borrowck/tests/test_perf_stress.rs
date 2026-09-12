use luna_borrowck::borrow_analysis::BorrowAnalyzer;
use luna_borrowck::dataflow::DataflowEngine;
use luna_borrowck::move_analysis::MoveAnalyzer;
use luna_mvir::{ValueOrigin, BasicBlock, Function, GlobalId, LabelId, Terminator, Operand};
use luna_semantic::ty::SemanticTypeId;

fn build_massive_graph(num_blocks: usize, deep_scc: bool) -> Function {
    let mut func = Function {
        name: GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        is_extern: false,
            is_async: false,
        ret_ty: SemanticTypeId(0),
        blocks: vec![],
        values: vec![],
    };

    func.values.push(luna_mvir::ValueData { span: None, origin: ValueOrigin::Temporary, inst: luna_mvir::Instruction::Alloca, ty: SemanticTypeId(0) });

    for i in 0..num_blocks {
        let block_name = format!("block_{}", i);
        let mut block = BasicBlock {
            label: LabelId { name: block_name },
            insts: vec![luna_mvir::ValueId(0)], // Push the Alloca to force state propagation
            terminator: None,
        };

        if i < num_blocks - 1 {
            // Forward edge
            if deep_scc && i % 10 == 0 && i > 0 {
                // Every 10th block, add a backedge to a previous block to create an SCC
                let back_target = format!("block_{}", i - 5);
                block.terminator = Some(Terminator::CondBr {
                    condition: Operand::Value(luna_mvir::ValueId(i as u32)),
                    true_target: LabelId { name: back_target },
                    false_target: LabelId { name: format!("block_{}", i + 1) },
                });
            } else {
                block.terminator = Some(Terminator::Br {
                    target: LabelId { name: format!("block_{}", i + 1) },
                });
            }
        } else {
            // Last block
            block.terminator = Some(Terminator::Ret { value: None });
        }

        func.blocks.push(block);
    }
    func
}

#[test]
fn test_perf_stress_move_analysis() {
    let func = build_massive_graph(1000, true);
    let mut analyzer = MoveAnalyzer::new(&func, None, None);
    
    let (_, iterations) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer);
    
    // We expect the number of iterations to be bounded, not infinite or excessively large
    // With 1000 blocks and some backedges, it might take a few passes.
    assert!(iterations < 5000, "MoveAnalysis took too many iterations: {}", iterations);
}

#[test]
fn test_perf_stress_borrow_analysis() {
    let func = build_massive_graph(1000, true);
    
    // Minimal mock live before for BorrowAnalyzer
    let live_before = std::collections::HashMap::new();
    let mut analyzer = BorrowAnalyzer::new(live_before, None, None, &func);
    
    let (_, iterations) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer);
    
    assert!(iterations < 5000, "BorrowAnalysis took too many iterations: {}", iterations);
}



