use mellis_borrowck::borrow_analysis::BorrowAnalyzer;
use mellis_mvir::*;
use mellis_semantic::{SemanticContext, SemanticTypeId, ty::{SemanticType, Mutability}};

fn make_func(name: &str, num_args: u32, instructions: Vec<Instruction>) -> Function {
    let mut func = Function {
        name: GlobalId { name: "test".to_string(), symbol_id: None },
        arg_count: num_args as usize,
        is_extern: false,
            is_async: false,
        blocks: vec![],
        values: vec![],
        ret_ty: SemanticTypeId(0),
    };

    let mut block = BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![],
        terminator: Some(Terminator::Ret { value: None }),
    };

    let mut val_id = 0;
    for _ in 0..num_args {
        func.values.push(ValueData { span: None, inst: Instruction::Alloca, ty: SemanticTypeId(0) });
        block.insts.push(ValueId(val_id));
        val_id += 1;
    }

    for inst in instructions {
        func.values.push(ValueData { span: None, inst, ty: SemanticTypeId(0) });
        block.insts.push(ValueId(val_id));
        val_id += 1;
    }

    func.blocks.push(block);
    func
}

#[test]
fn test_ffi_escape_then_reentrant_callback_conflict() {
    let mut func = make_func("test", 1, vec![
        // v1 = &mut arg0
        Instruction::Borrow { is_rw: true, base: Operand::Value(ValueId(0)) },
        // ffi_register_callback(v1) -> this passes the mutable pointer to C, where it escapes (MayEscape).
        Instruction::CallDirect {
            callee: GlobalId { name: "ffi_register_callback".to_string(), symbol_id: None },
            args: vec![Operand::Value(ValueId(1))]
        },
        
        // --- C-side holds the pointer (MayEscape) ---
        // ... sometime later, C-side calls back into Mellis or a thread is running ...

        // ffi_invoke_callback() -> simulate the event firing or some other function call
        Instruction::CallDirect {
            callee: GlobalId { name: "ffi_invoke_callback".to_string(), symbol_id: None },
            args: vec![]
        },

        // --- Mellis-side tries to read `arg0` concurrently ---
        // Because `v1` escaped into C space and hasn't been explicitly released, the mutable loan on `arg0`
        // MUST still be active. This Load is an aliasing violation (Mellis reads while C-side holds mut ptr).
        Instruction::Load { ptr: Operand::Value(ValueId(0)) }, 
    ]);
    
    let mut ctx = SemanticContext::new();
    let mut_ptr_ty = ctx.types.intern(SemanticType::Pointer(Mutability::Mutable, SemanticTypeId(3)));
    func.values[1].ty = mut_ptr_ty; // v1 is *mut T
    
    let diagnostics = BorrowAnalyzer::analyze(&func, None, Some(&ctx));
    
    // We expect exactly 1 conflict because the FFI pointer escaped and poisoned the value.
    assert_eq!(diagnostics.len(), 1, "Should conflict because the mutable pointer escaped into FFI and is considered held indefinitely");
    assert!(diagnostics[0].message.contains("Cannot access"), "Expected read/write conflict due to active MayEscape loan");
}

#[test]
fn test_ffi_escape_negative_control() {
    let mut func = make_func("test", 1, vec![
        // v1 = &mut arg0
        Instruction::Borrow { is_rw: true, base: Operand::Value(ValueId(0)) },
        // ffi_register_callback(v1)
        Instruction::CallDirect {
            callee: GlobalId { name: "ffi_register_callback".to_string(), symbol_id: None },
            args: vec![Operand::Value(ValueId(1))]
        },
        // NO ffi_invoke_callback() here! Direct Load.
        Instruction::Load { ptr: Operand::Value(ValueId(0)) }, 
    ]);
    
    let mut ctx = SemanticContext::new();
    let mut_ptr_ty = ctx.types.intern(SemanticType::Pointer(Mutability::Mutable, SemanticTypeId(3)));
    func.values[1].ty = mut_ptr_ty; // v1 is *mut T
    
    let diagnostics = BorrowAnalyzer::analyze(&func, None, Some(&ctx));
    
    // We expect the result to be IDENTICAL to the reentrant test
    assert_eq!(diagnostics.len(), 1, "Should have 1 conflict exactly like the reentrant test");
    assert!(diagnostics[0].message.contains("Cannot access"), "Expected read/write conflict due to active MayEscape loan");
}



