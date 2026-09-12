use luna_borrowck::borrow_check_function;
use luna_common::ids::{Span, SymbolId, SyntaxContext};
use luna_ast::Visibility;
use luna_mvir::*;
use luna_semantic::{
    symbol::{ScopeId, Symbol, SymbolKind},
    SemanticContext, SemanticType, SemanticTypeId,
};
use std::collections::HashMap;

#[test]
fn test_reject_partial_move_under_user_drop() {
    let mut ctx = SemanticContext::new();
    let struct_sym = SymbolId(ctx.symbol_table.symbols.len() as u32);
    let drop_meth_sym = SymbolId(struct_sym.0 + 1);

    // Register struct symbol
    ctx.symbol_table.symbols.push(Symbol {
        id: struct_sym,
        name: "HasDropResource".to_string(),
        ctxt: SyntaxContext::ROOT,
        kind: SymbolKind::Struct,
        scope: ScopeId(0),
        span: Span::default(),
        visibility: Visibility::Public,
        decl_id: None,
        inner_scope: None,
        provider_id: None,
    });
    let struct_ty = ctx.types.intern(SemanticType::Struct(struct_sym, Vec::new(), Vec::new()));

    // Register User Drop implementation (RFC P6 MOVE-PARTIAL-1)
    ctx.tables.drop_impls.insert(struct_sym, drop_meth_sym);

    let mut func = Function {
        name: GlobalId {
            name: "test_partial_move_reject".to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 1,
        link_name: None,
        param_types: vec![struct_ty],
        values: Vec::new(),
        blocks: Vec::new(),
    };

    // v0 = Alloca (param 0, struct HasDropResource)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Parameter(0),
        inst: Instruction::Alloca,
        ty: struct_ty,
    });

    // v1 = Extract field 0 (moving out of a struct with user drop)
    let field_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::String));
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Extract {
            value: Operand::Value(ValueId(0)),
            variant_idx: 0,
            field_idx: 0,
        },
        ty: field_ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _) = borrow_check_function(&func, &ctx, &HashMap::new());

    assert!(
        diagnostics.iter().any(|d| d.message.contains("Cannot move out of a subplace") || d.message.contains("E_PARTIAL_MOVE_UNDER_DROP")),
        "Expected PartialMoveUnderDrop for type implementing Drop, but got: {:?}",
        diagnostics
    );
}

#[test]
fn test_allow_partial_move_without_user_drop() {
    let mut ctx = SemanticContext::new();
    let struct_sym = SymbolId(ctx.symbol_table.symbols.len() as u32);

    // Register struct symbol WITHOUT drop_impls (RFC P6 MOVE-PARTIAL-2)
    ctx.symbol_table.symbols.push(Symbol {
        id: struct_sym,
        name: "NoDropPair".to_string(),
        ctxt: SyntaxContext::ROOT,
        kind: SymbolKind::Struct,
        scope: ScopeId(0),
        span: Span::default(),
        visibility: Visibility::Public,
        decl_id: None,
        inner_scope: None,
        provider_id: None,
    });
    let struct_ty = ctx.types.intern(SemanticType::Struct(struct_sym, Vec::new(), Vec::new()));

    let mut func = Function {
        name: GlobalId {
            name: "test_partial_move_allow".to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 1,
        link_name: None,
        param_types: vec![struct_ty],
        values: Vec::new(),
        blocks: Vec::new(),
    };

    // v0 = Alloca (param 0, struct NoDropPair)
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Parameter(0),
        inst: Instruction::Alloca,
        ty: struct_ty,
    });

    // v1 = Extract field 0
    let field_ty = ctx.types.intern(SemanticType::Primitive(luna_semantic::ty::BuiltinType::String));
    func.values.push(ValueData {
        span: None,
        origin: ValueOrigin::Temporary,
        inst: Instruction::Extract {
            value: Operand::Value(ValueId(0)),
            variant_idx: 0,
            field_idx: 0,
        },
        ty: field_ty,
    });

    func.blocks.push(BasicBlock {
        label: LabelId {
            name: "entry".to_string(),
        },
        insts: vec![ValueId(0), ValueId(1)],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let (diagnostics, _) = borrow_check_function(&func, &ctx, &HashMap::new());

    assert!(
        !diagnostics.iter().any(|d| d.message.contains("Cannot move out of a subplace") || d.message.contains("E_PARTIAL_MOVE_UNDER_DROP")),
        "Did not expect PartialMoveUnderDrop for struct without Drop impl, but got: {:?}",
        diagnostics
    );
}
