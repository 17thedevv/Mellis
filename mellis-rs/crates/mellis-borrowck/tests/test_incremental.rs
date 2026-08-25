use mellis_borrowck::borrow_analysis::BorrowAnalyzer;
use mellis_borrowck::dataflow::DataflowEngine;
use mellis_borrowck::move_analysis::MoveAnalyzer;
use mellis_mvir::{BasicBlock, Function, GlobalId, Instruction, LabelId, Terminator, Operand, ValueId, ValueData};
use mellis_semantic::ty::SemanticTypeId;

fn make_base_graph() -> Function {
    let mut func = Function {
        name: GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: 0,
        is_extern: false,
        ret_ty: SemanticTypeId(0),
        blocks: vec![],
        values: vec![],
    };

    // A small CFG: entry -> block1 -> block2 -> block1 -> exit
    
    // Var 0
    func.values.push(ValueData { inst: Instruction::Alloca, ty: SemanticTypeId(0) });
    
    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0)],
        terminator: Some(Terminator::Br { target: LabelId { name: "block1".to_string() } }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "block1".to_string() },
        insts: vec![],
        terminator: Some(Terminator::CondBr {
            condition: Operand::Value(ValueId(0)),
            true_target: LabelId { name: "block2".to_string() },
            false_target: LabelId { name: "exit".to_string() },
        }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "block2".to_string() },
        insts: vec![],
        terminator: Some(Terminator::Br { target: LabelId { name: "block1".to_string() } }),
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "exit".to_string() },
        insts: vec![],
        terminator: Some(Terminator::Ret { value: None }),
    });

    func
}

#[test]
fn test_incremental_move_analysis() {
    let mut func = make_base_graph();
    
    // Result A (Base run)
    let mut analyzer = MoveAnalyzer::new();
    let (states_a, iterations_a) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer);

    // Modify the CFG (add a read/move instruction in block2)
    // var 1
    func.values.push(ValueData { inst: Instruction::Eq { left: Operand::Value(ValueId(0)), right: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0) });
    let block2_idx = func.blocks.iter().position(|b| b.label.name == "block2").unwrap();
    func.blocks[block2_idx].insts.push(ValueId(1));

    // Result B (Incremental)
    let mut analyzer_inc = MoveAnalyzer::new();
    let (states_b, iterations_b) = DataflowEngine::run_forward_incremental(&func, &mut analyzer_inc, states_a.clone());

    // Result C (Full from scratch)
    let mut analyzer_full = MoveAnalyzer::new();
    let (states_c, iterations_c) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer_full);

    // Assert incremental is correct
    assert_eq!(states_b, states_c, "Incremental recomputation states must exactly match full recomputation");

    // Assert incremental is faster (fewer iterations)
    // For MoveAnalysis on this tiny graph, iterations might be very low, but incremental should take at most full iterations.
    // Generally incremental iterations < full iterations, except for trivial graphs where both are 1-2.
    assert!(iterations_b <= iterations_c, "Incremental iterations ({}) should be <= full iterations ({})", iterations_b, iterations_c);
}

#[test]
fn test_incremental_borrow_analysis() {
    let mut func = make_base_graph();
    let live_before = std::collections::HashMap::new();

    // Result A (Base run)
    let mut analyzer = BorrowAnalyzer::new(live_before.clone(), None, None, &func);
    let (states_a, iterations_a) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer);

    // Modify the CFG (add a borrow in block2)
    // var 1
    func.values.push(ValueData { inst: Instruction::Borrow { is_rw: true, base: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0) });
    let block2_idx = func.blocks.iter().position(|b| b.label.name == "block2").unwrap();
    func.blocks[block2_idx].insts.push(ValueId(1));

    // Result B (Incremental)
    let mut analyzer_inc = BorrowAnalyzer::new(live_before.clone(), None, None, &func);
    let (states_b, iterations_b) = DataflowEngine::run_forward_incremental(&func, &mut analyzer_inc, states_a.clone());

    // Result C (Full from scratch)
    let mut analyzer_full = BorrowAnalyzer::new(live_before.clone(), None, None, &func);
    let (states_c, iterations_c) = DataflowEngine::run_forward_with_stats(&func, &mut analyzer_full);

    assert_eq!(states_b, states_c, "Incremental recomputation states must exactly match full recomputation");
    assert!(iterations_b <= iterations_c, "Incremental iterations ({}) should be <= full iterations ({})", iterations_b, iterations_c);
}



