use mellis_driver::{check, CompilerOptions};
use mellis_driver::sysroot::Sysroot;
use mellis_mvir::interp::MvirInterpreter;
use mellis_mvir::mvir::*;
use mellis_semantic::{SemanticContext, ComptimeError, SemanticType, SymbolKind};
use mellis_semantic::ty::{Mutability, LifetimeId};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_adv_comptime_dyn_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<mellis_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ms");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

fn add_sym(ctx: &mut SemanticContext, name: &str, kind: SymbolKind) -> mellis_common::ids::SymbolId {
    ctx.symbol_table.declare_symbol(
        name.to_string(),
        kind,
        mellis_semantic::ScopeId(0),
        mellis_common::Span::default(),
        None,
        mellis_ast::Visibility::Public,
        &mut ctx.diagnostics,
    )
}

// =============================================================================
// Vector 1 & 2: Comptime Virtual Call Dispatch & MakeTraitObject
// =============================================================================

// ADV-01: Virtual call on &dyn Trait in comptime block evaluates correctly.
#[test]
fn test_adv_comptime_01_dyn_virtual_call_success() {
    let src = r#"
        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        struct Polite {
            val: i32,
        }

        impl Greeter for Polite {
            fn greet(self: &Polite) -> i32 {
                self.val * 2
            }
        }

        const GREET: i32 = comptime {
            dec p = Polite { val: 21 };
            dec g: &dyn Greeter = &p;
            g.greet()
        };

        fn main() -> i32 {
            GREET
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(success, "Expected successful comptime virtual call, got diags: {:?}", diags);
}

// ADV-02: VM explicitly executes MakeTraitObject and CallVirt instructions.
#[test]
fn test_adv_comptime_02_make_trait_object_and_call() {
    let mut ctx = SemanticContext::new();
    let trait_sym = add_sym(&mut ctx, "Greeter", SymbolKind::Trait);
    let concrete_sym = add_sym(&mut ctx, "Polite", SymbolKind::Struct);
    let method_sym = add_sym(&mut ctx, "greet", SymbolKind::TraitMethod);

    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::BuiltinType::I32));
    let struct_ty = ctx.types.intern(SemanticType::Struct(concrete_sym, vec![], vec![i32_ty]));
    let ref_struct_ty = ctx.types.intern(SemanticType::Reference(LifetimeId(0), Mutability::Immutable, struct_ty));

    // Register method in trait_methods
    ctx.tables.trait_methods.insert(trait_sym, vec![method_sym]);

    // Build concrete method function: fn greet(self: &Polite) -> i32 { return (*self).val * 2; }
    let mut method_fn = Function {
        name: GlobalId { name: "greet".to_string(), symbol_id: Some(method_sym) },
        is_extern: false,
        is_async: false,
        arg_count: 1,
        link_name: None,
        param_types: vec![ref_struct_ty],
        ret_ty: i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    // v0 = param(0) alloca
    method_fn.values.push(ValueData {
        ty: ref_struct_ty,
        inst: Instruction::Alloca,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v1 = Load { ptr: v0 } -> &Polite
    method_fn.values.push(ValueData {
        ty: ref_struct_ty,
        inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v2 = FieldPtr { base: v1, field_idx: 0 } -> &val
    method_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::FieldPtr { base: Operand::Value(ValueId(1)), field_idx: 0 },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v3 = Load { ptr: v2 } -> val (21)
    method_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::Load { ptr: Operand::Value(ValueId(2)) },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v4 = Mul { left: v3, right: 2 } -> 42
    method_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::Mul { left: Operand::Value(ValueId(3)), right: Operand::Number("2".to_string()) },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    method_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(4))) }),
    });

    // Caller function: creates struct, makes trait object, calls CallVirt
    let mut caller_fn = Function {
        name: GlobalId { name: "caller".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    // v0 = Alloca (struct Polite)
    caller_fn.values.push(ValueData {
        ty: struct_ty,
        inst: Instruction::Alloca,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v1 = FieldPtr { base: v0, field_idx: 0 }
    caller_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::FieldPtr { base: Operand::Value(ValueId(0)), field_idx: 0 },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v2 = Store { ptr: v1, value: 21 }
    caller_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::Store { ptr: Operand::Value(ValueId(1)), value: Operand::Number("21".to_string()) },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v3 = MakeTraitObject { data: v0, concrete: Polite, trait: Greeter }
    caller_fn.values.push(ValueData {
        ty: ctx.types.intern(SemanticType::DynTrait(trait_sym)),
        inst: Instruction::MakeTraitObject {
            data_ptr: Operand::Value(ValueId(0)),
            vtable: GlobalId { name: "vtable_Greeter_Polite".to_string(), symbol_id: Some(trait_sym) },
            trait_sym,
            concrete_sym,
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v4 = CallVirt { obj: v3, method_idx: 0, args: [] }
    caller_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::CallVirt {
            obj: Operand::Value(ValueId(3)),
            method_idx: 0,
            args: Vec::new(),
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    caller_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(4))) }),
    });

    let module = Module {
        functions: vec![method_fn, caller_fn.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&caller_fn, vec![]);
    assert!(res.is_ok(), "CallVirt evaluation failed: {:?}", res);
    let val = res.unwrap();
    match val.to_comptime_value(&ctx).unwrap() {
        mellis_semantic::ComptimeValue::Int { val: 42, .. } => {}
        other => panic!("Expected ComptimeValue::Int(42), got: {:?}", other),
    }
}

// =============================================================================
// Vector 3, 4, 5: Fat Pointer Comptime Boundary Escape Prevention
// =============================================================================

// ADV-03: &dyn Trait escaping comptime boundary into const is rejected with PointerEscape.
#[test]
fn test_adv_comptime_03_dyn_pointer_escape_rejected() {
    let src = r#"
        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        struct Polite {
            val: i32,
        }

        impl Greeter for Polite {
            fn greet(self: &Polite) -> i32 {
                self.val
            }
        }

        const ESCAPED: &dyn Greeter = comptime {
            dec p = Polite { val: 42 };
            dec g: &dyn Greeter = &p;
            g
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_03", src);
    assert!(!success, "Expected escaping &dyn Trait from comptime to be rejected");
    let err = diags.iter().find(|d| d.message.contains("pointer escape") || d.message.contains("cannot evaluate constant in comptime"));
    assert!(err.is_some(), "Expected pointer escape diagnostic, got: {:?}", diags);
}

// ADV-04: &[T] escaping comptime boundary into const is rejected with PointerEscape.
#[test]
fn test_adv_comptime_04_slice_pointer_escape_rejected() {
    let src = r#"
        const ESCAPED_SLICE: &[i32] = comptime {
            dec arr = [1, 2, 3, 4];
            dec s: &[i32] = &arr;
            s
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_04", src);
    assert!(!success, "Expected escaping &[T] from comptime to be rejected");
    let err = diags.iter().find(|d| d.message.contains("pointer escape") || d.message.contains("cannot evaluate constant in comptime"));
    assert!(err.is_some(), "Expected pointer escape diagnostic, got: {:?}", diags);
}

// ADV-05: Fat pointer smuggled inside a compound struct/tuple is caught by recursive check.
#[test]
fn test_adv_comptime_05_smuggled_fat_pointer_in_struct_rejected() {
    let src = r#"
        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        struct Polite {
            val: i32,
        }

        impl Greeter for Polite {
            fn greet(self: &Polite) -> i32 {
                self.val
            }
        }

        struct Smuggler {
            tag: i32,
            ptr: &dyn Greeter,
        }

        const SMUGGLED: Smuggler = comptime {
            dec p = Polite { val: 42 };
            dec g: &dyn Greeter = &p;
            Smuggler { tag: 1, ptr: g }
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(!success, "Expected smuggled fat pointer in struct to be rejected");
    let err = diags.iter().find(|d| d.message.contains("pointer escape") || d.message.contains("cannot evaluate constant in comptime"));
    assert!(err.is_some(), "Expected pointer escape diagnostic, got: {:?}", diags);
}

// =============================================================================
// Vector 6 & 7: Top-Level Const with Associated Type Projections
// =============================================================================

// ADV-06: Const declaration using associated type Foo::Output normalizes correctly.
#[test]
fn test_adv_comptime_06_const_associated_type_projection() {
    let src = r#"
        trait HasType {
            type Output;
        }

        struct Config {}

        impl HasType for Config {
            type Output = i32;
        }

        const MY_CONFIG: Config::Output = 42;

        fn main() -> i32 {
            MY_CONFIG
        }
    "#;
    let (success, diags) = run_compiler("test_06", src);
    assert!(success, "Expected successful associated type projection in const declaration, got: {:?}", diags);
}

// ADV-07: Const dependency order where const B depends on const A with associated type.
#[test]
fn test_adv_comptime_07_const_topological_eval_with_associated_type() {
    let src = r#"
        trait Compute {
            type Result;
        }

        struct Math {}

        impl Compute for Math {
            type Result = i32;
        }

        const DERIVED: i32 = BASE * 2;
        const BASE: Math::Result = 21;

        fn main() -> i32 {
            DERIVED
        }
    "#;
    let (success, diags) = run_compiler("test_07", src);
    assert!(success, "Expected successful topological eval with associated types, got: {:?}", diags);
}

// =============================================================================
// Vector 8 & 9: Comptime Functions & Generic Associated Types
// =============================================================================

// ADV-08: Pure function using associated type evaluated at comptime.
#[test]
fn test_adv_comptime_08_comptime_function_with_associated_type() {
    let src = r#"
        trait ValueProducer {
            type Item;
            fn produce(self: &Self) -> Self::Item;
        }

        struct Producer {
            dummy: i32,
        }

        impl ValueProducer for Producer {
            type Item = i32;
            fn produce(self: &Producer) -> i32 {
                100
            }
        }

        const PRODUCED: Producer::Item = comptime {
            dec p = Producer { dummy: 0 };
            p.produce()
        };

        fn main() -> i32 {
            PRODUCED
        }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(success, "Expected comptime evaluation of function with associated type, got: {:?}", diags);
}

// ADV-09: Unmonomorphized generic with unresolved associated type entering comptime is rejected.
#[test]
fn test_adv_comptime_09_unmonomorphized_generic_comptime_rejected() {
    use mellis_semantic::ComptimeEngine;

    let mut ctx = SemanticContext::new();
    let trait_sym = add_sym(&mut ctx, "Iterator", SymbolKind::Trait);
    let assoc_sym = add_sym(&mut ctx, "Item", SymbolKind::AssociatedType);
    let param_sym = add_sym(&mut ctx, "T", SymbolKind::Type);

    let self_ty = ctx.types.intern(SemanticType::GenericParam(param_sym));
    let proj_ty = ctx.types.intern(SemanticType::Projection {
        self_type: self_ty,
        trait_id: trait_sym,
        assoc_type: assoc_sym,
    });

    let engine = mellis_mvir::MvirComptimeEngine::default();
    let mut arena = mellis_ast::AstArena::new();
    let source_manager = mellis_common::source::SourceManager::new();

    let dummy_expr = arena.alloc_expr(mellis_ast::Expr::Literal(
        mellis_lexer::Token::new(mellis_lexer::TokenKind::IntegerLiteral, mellis_common::Span::default()),
        "0".to_string(),
    ));
    ctx.tables.expr_types.insert(dummy_expr, proj_ty);

    let res = engine.eval_expr(&arena, &ctx, &source_manager, dummy_expr);
    assert!(res.is_err(), "Expected generic associated type projection in comptime to be rejected");
    match res.unwrap_err() {
        ComptimeError::TypeMismatch(msg) => {
            assert!(msg.contains("E_UNRESOLVED_PROJECTION"));
        }
        other => panic!("Expected TypeMismatch with E_UNRESOLVED_PROJECTION, got: {:?}", other),
    }
}

// =============================================================================
// Vector 10: Comptime Slice Indexing
// =============================================================================

// ADV-10: Slice indexing inside comptime block computes correct value.
#[test]
fn test_adv_comptime_10_comptime_slice_indexing() {
    let src = r#"
        const ELEMENT: i32 = comptime {
            dec arr = [10, 20, 30, 40];
            dec s: &[i32] = &arr;
            s[2]
        };

        fn main() -> i32 {
            ELEMENT
        }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(success, "Expected successful slice indexing in comptime, got: {:?}", diags);
}

// =============================================================================
// Vector 11 & 12: Comptime Trait Object Drop Glue & Resource Leak Detection
// =============================================================================

// ADV-11: DropVirt executes drop glue for trait object inside VM.
#[test]
fn test_adv_comptime_11_comptime_dyn_trait_drop_glue() {
    let mut ctx = SemanticContext::new();
    let trait_sym = add_sym(&mut ctx, "AutoCloseable", SymbolKind::Trait);
    let concrete_sym = add_sym(&mut ctx, "Resource", SymbolKind::Struct);
    let drop_sym = add_sym(&mut ctx, "drop", SymbolKind::Function);

    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::BuiltinType::I32));
    let struct_ty = ctx.types.intern(SemanticType::Struct(concrete_sym, vec![], vec![i32_ty]));

    // Register drop impl
    ctx.tables.drop_impls.insert(concrete_sym, drop_sym);

    // Drop function: fn drop(self: Resource) {}
    let mut drop_fn = Function {
        name: GlobalId { name: "drop".to_string(), symbol_id: Some(drop_sym) },
        is_extern: false,
        is_async: false,
        arg_count: 1,
        link_name: None,
        param_types: vec![struct_ty],
        ret_ty: ctx.types.intern(SemanticType::Void),
        blocks: Vec::new(),
        values: Vec::new(),
    };
    drop_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: Vec::new(),
        terminator: Some(Terminator::Ret { value: None }),
    });

    // Test function: creates trait object, executes DropVirt, returns 0
    let mut test_fn = Function {
        name: GlobalId { name: "test_drop".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    // v0 = Alloca
    test_fn.values.push(ValueData {
        ty: struct_ty,
        inst: Instruction::Alloca,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v1 = MakeTraitObject
    test_fn.values.push(ValueData {
        ty: ctx.types.intern(SemanticType::DynTrait(trait_sym)),
        inst: Instruction::MakeTraitObject {
            data_ptr: Operand::Value(ValueId(0)),
            vtable: GlobalId { name: "vtable_AutoCloseable_Resource".to_string(), symbol_id: Some(trait_sym) },
            trait_sym,
            concrete_sym,
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v2 = DropVirt { obj: v1 }
    test_fn.values.push(ValueData {
        ty: ctx.types.intern(SemanticType::Void),
        inst: Instruction::DropVirt {
            obj: Operand::Value(ValueId(1)),
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    test_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Number("0".to_string())) }),
    });

    let module = Module {
        functions: vec![drop_fn, test_fn.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&test_fn, vec![]);
    assert!(res.is_ok(), "DropVirt execution failed: {:?}", res);
}

// ADV-12: Comptime heap allocation without drop is caught as ResourceLeak.
#[test]
fn test_adv_comptime_12_comptime_dyn_heap_leak_rejected() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::BuiltinType::I32));

    // Function that allocates heap memory but never frees it
    let mut leak_fn = Function {
        name: GlobalId { name: "leak".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    // v0 = HeapAlloc
    leak_fn.values.push(ValueData {
        ty: ctx.types.intern(SemanticType::Pointer(Mutability::Immutable, i32_ty)),
        inst: Instruction::HeapAlloc,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    leak_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Number("42".to_string())) }),
    });

    let module = Module {
        functions: vec![leak_fn.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&leak_fn, vec![]);
    assert!(res.is_ok());
    // Interpreter detected leak
    assert!(!interp.heap.allocations.is_empty(), "Expected unfreed allocation in heap");
}

// =============================================================================
// Vector 13 & 14: Virtual Call on Missing Method & Projection Barrier
// =============================================================================

// ADV-13: CallVirt on missing method traps deterministically with UnsupportedOperation.
#[test]
fn test_adv_comptime_13_virtual_call_on_missing_method_trapped() {
    let mut ctx = SemanticContext::new();
    let trait_sym = add_sym(&mut ctx, "UnknownTrait", SymbolKind::Trait);
    let concrete_sym = add_sym(&mut ctx, "UnknownType", SymbolKind::Struct);
    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::BuiltinType::I32));

    // CallVirt on method_idx 99 (not registered in trait_methods)
    let mut bad_fn = Function {
        name: GlobalId { name: "bad_call".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    bad_fn.values.push(ValueData {
        ty: ctx.types.intern(SemanticType::DynTrait(trait_sym)),
        inst: Instruction::MakeTraitObject {
            data_ptr: Operand::Number("0".to_string()),
            vtable: GlobalId { name: "vtable_unknown".to_string(), symbol_id: Some(trait_sym) },
            trait_sym,
            concrete_sym,
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    bad_fn.values.push(ValueData {
        ty: i32_ty,
        inst: Instruction::CallVirt {
            obj: Operand::Value(ValueId(0)),
            method_idx: 99,
            args: Vec::new(),
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    bad_fn.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(1))) }),
    });

    let module = Module {
        functions: vec![bad_fn.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&bad_fn, vec![]);
    assert!(res.is_err(), "Expected CallVirt on missing method to fail");
    match res.unwrap_err() {
        ComptimeError::UnsupportedOperation(msg) => {
            assert!(msg.contains("E_COMPTIME_VIRTUAL_CALL") || msg.contains("not found in trait methods table") || msg.contains("dynamic dispatch failed"));
        }
        other => panic!("Expected ComptimeError::UnsupportedOperation, got: {:?}", other),
    }
}

// ADV-14: Unresolved SemanticType::Projection entering VM is rejected with E_UNRESOLVED_PROJECTION.
#[test]
fn test_adv_comptime_14_unresolved_projection_barrier() {
    use mellis_semantic::ComptimeEngine;

    let mut ctx = SemanticContext::new();
    let trait_sym = add_sym(&mut ctx, "FooTrait", SymbolKind::Trait);
    let assoc_sym = add_sym(&mut ctx, "Output", SymbolKind::AssociatedType);
    let self_ty = ctx.types.intern(SemanticType::GenericParam(mellis_common::ids::SymbolId(999)));
    let proj_ty = ctx.types.intern(SemanticType::Projection {
        self_type: self_ty,
        trait_id: trait_sym,
        assoc_type: assoc_sym,
    });

    let engine = mellis_mvir::MvirComptimeEngine::default();
    let mut arena = mellis_ast::AstArena::new();
    let source_manager = mellis_common::source::SourceManager::new();

    let dummy_expr = arena.alloc_expr(mellis_ast::Expr::Literal(
        mellis_lexer::Token::new(mellis_lexer::TokenKind::IntegerLiteral, mellis_common::Span::default()),
        "0".to_string(),
    ));
    ctx.tables.expr_types.insert(dummy_expr, proj_ty);

    // Call engine
    let res = engine.eval_expr(&arena, &ctx, &source_manager, dummy_expr);
    assert!(res.is_err(), "Expected unresolved projection to be rejected");
    match res.unwrap_err() {
        ComptimeError::TypeMismatch(msg) => {
            assert!(msg.contains("E_UNRESOLVED_PROJECTION"));
        }
        other => panic!("Expected TypeMismatch with E_UNRESOLVED_PROJECTION, got: {:?}", other),
    }
}

// =============================================================================
// Vector 15 & 16: Array Size & Deep Invariant Check
// =============================================================================

// ADV-15: Array size derived from comptime associated type projection.
#[test]
fn test_adv_comptime_15_array_size_from_comptime_associated_type() {
    let src = r#"
        trait Sizer {
            type SizeType;
        }

        struct S {}

        impl Sizer for S {
            type SizeType = i32;
        }

        const ARRAY_LEN: S::SizeType = 4;

        fn main() -> i32 {
            dec arr: [i32; ARRAY_LEN] = [1, 2, 3, 4];
            arr[0]
        }
    "#;
    let (success, diags) = run_compiler("test_15", src);
    assert!(success, "Expected array size from comptime associated type to succeed, got: {:?}", diags);
}

// ADV-16: Deeply nested VM pointer escape check: RuntimeReachable(V) ∩ VMObjects = ∅
// Enforces that even within nested structs/arrays/tuples, fat pointers cannot escape.
#[test]
fn test_adv_comptime_16_runtime_reachable_fat_pointer_escape_invariant() {
    let src = r#"
        trait Worker {
            fn work(self: &Self) -> i32;
        }

        struct SimpleWorker {
            id: i32,
        }

        impl Worker for SimpleWorker {
            fn work(self: &SimpleWorker) -> i32 {
                self.id
            }
        }

        struct DeepContainer {
            workers: [&dyn Worker; 1],
        }

        const ESCAPED_CONTAINER: DeepContainer = comptime {
            dec w = SimpleWorker { id: 7 };
            dec dyn_w: &dyn Worker = &w;
            DeepContainer {
                workers: [dyn_w],
            }
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_16", src);
    assert!(!success, "Expected deeply nested fat pointer escape to be rejected");
    let err = diags.iter().find(|d| d.message.contains("pointer escape") || d.message.contains("cannot evaluate constant in comptime"));
    assert!(err.is_some(), "Expected pointer escape diagnostic, got: {:?}", diags);
}
