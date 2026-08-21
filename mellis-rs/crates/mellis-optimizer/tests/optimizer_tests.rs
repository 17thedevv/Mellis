use mellis_mvir::{Module, Function, BasicBlock, ValueData, Instruction, Terminator, Operand, ValueId, GlobalId, LabelId};
use mellis_semantic::SemanticTypeId;
use mellis_common::ids::SymbolId;
use mellis_optimizer::{PassManager, ConstantFolding, DeadCodeElimination, verify_module};

fn dummy_module() -> Module {
    let mut func = Function {
        name: GlobalId { name: "test_func".to_string(), symbol_id: Some(SymbolId(0)) },
        ret_ty: SemanticTypeId(0),
        values: Vec::new(),
        blocks: Vec::new(),
    };
    
    // v0 = const 2
    func.values.push(ValueData { inst: Instruction::Assign(Operand::Number("2".to_string())), ty: SemanticTypeId(0) });
    // v1 = const 3
    func.values.push(ValueData { inst: Instruction::Assign(Operand::Number("3".to_string())), ty: SemanticTypeId(0) });
    // v2 = add v0, v1 (can be constant folded to 5)
    func.values.push(ValueData { 
        inst: Instruction::Add { left: Operand::Value(ValueId(0)), right: Operand::Value(ValueId(1)) }, 
        ty: SemanticTypeId(0) 
    });
    // v3 = add v2, 10 (can be folded to 15, then DCE removes v0, v1, v2 if not returned)
    func.values.push(ValueData { 
        inst: Instruction::Add { left: Operand::Value(ValueId(2)), right: Operand::Number("10".to_string()) }, 
        ty: SemanticTypeId(0) 
    });
    // v4 = return v3
    
    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry0".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(3))) }),
    });

    let mut module = Module::new();
    module.functions.push(func);
    module
}

#[test]
fn test_verifier() {
    let module = dummy_module();
    assert!(verify_module(&module).is_ok());
}

#[test]
fn test_constant_folding() {
    let mut module = dummy_module();
    let mut pm = PassManager::new();
    pm.add_pass(Box::new(ConstantFolding::new()));
    
    pm.run(&mut module);
    
    let func = &module.functions[0];
    
    // Check if v2 is now Assign(5)
    match &func.value(ValueId(2)).inst {
        Instruction::Assign(Operand::Number(n)) => assert_eq!(n, "5"),
        _ => panic!("Expected v2 to be Assign(5)"),
    }
    
    // Check if v3 is now Assign(15)
    match &func.value(ValueId(3)).inst {
        Instruction::Assign(Operand::Number(n)) => assert_eq!(n, "15"),
        _ => panic!("Expected v3 to be Assign(15)"),
    }
}

#[test]
fn test_dce() {
    let mut module = dummy_module();
    let mut pm = PassManager::new();
    pm.add_pass(Box::new(ConstantFolding::new()));
    pm.add_pass(Box::new(DeadCodeElimination::new()));
    
    pm.run(&mut module);
    
    let func = &module.functions[0];
    let block = &func.blocks[0];
    
    // After constant folding, v3 is Assign(15). 
    // v3 is returned. 
    // v0, v1, v2 are unused.
    // So block should only contain v3!
    assert_eq!(block.insts.len(), 1);
    assert_eq!(block.insts[0], ValueId(3));
}
