use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_common::ids::SymbolId;
use mellis_common::Span;
use mellis_ast::Visibility;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::symbol::{ProviderId, ScopeId, ScopeKind, SymbolKind};
use mellis_semantic::ty::SemanticType;
use mellis_semantic::SemanticContext;

fn run_semantic(source: &str) -> (SemanticContext, bool) {
    run_semantic_with_setup(source, |_| {})
}

fn run_semantic_with_setup<F>(source: &str, setup: F) -> (SemanticContext, bool)
where
    F: FnOnce(&mut SemanticContext),
{
    run_semantic_with_hook(source, setup, |_| {})
}

fn run_semantic_with_hook<F1, F2>(source: &str, setup: F1, post_resolve: F2) -> (SemanticContext, bool)
where
    F1: FnOnce(&mut SemanticContext),
    F2: FnOnce(&mut SemanticContext),
{
    let mut source_manager = mellis_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), source.to_string());
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = match parser.parse_file() {
        Ok(items) => items,
        Err(e) => panic!("parse_file failed: {:?}", e),
    };

    let mut ctx = SemanticContext::new();
    setup(&mut ctx);

    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&items);

    if ctx.diagnostics.iter().any(|d| matches!(d.level, mellis_common::DiagnosticLevel::Error)) {
        return (ctx, false);
    }

    post_resolve(&mut ctx);

    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&items);
    let success = !ctx.diagnostics.iter().any(|d| matches!(d.level, mellis_common::DiagnosticLevel::Error));
    (ctx, success)
}

fn setup_core_provider(ctx: &mut SemanticContext) -> (ScopeId, SymbolId, SymbolId, SymbolId, SymbolId) {
    let global_scope = ScopeId(0);
    let core_provider = ProviderId(1);
    let core_scope = ctx.symbol_table.create_scope(ScopeKind::Module, Some(global_scope));

    let core_mod_sym = ctx.symbol_table.declare_symbol(
        "core".to_string(),
        SymbolKind::Module,
        global_scope,
        Span::default(),
        None,
        Visibility::Public,
        &mut ctx.diagnostics,
    );
    ctx.symbol_table.symbols[core_mod_sym.0 as usize].provider_id = Some(core_provider);
    ctx.symbol_table.set_inner_scope(core_mod_sym, core_scope);
    ctx.external_module_scopes.insert("core".to_string(), core_scope);

    // Foreign Trait: Clone
    let clone_sym = ctx.symbol_table.declare_symbol(
        "Clone".to_string(),
        SymbolKind::Trait,
        core_scope,
        Span::default(),
        None,
        Visibility::Public,
        &mut ctx.diagnostics,
    );
    ctx.symbol_table.symbols[clone_sym.0 as usize].provider_id = Some(core_provider);

    // Foreign Trait: FromResidual
    let from_residual_sym = ctx.symbol_table.declare_symbol(
        "FromResidual".to_string(),
        SymbolKind::Trait,
        core_scope,
        Span::default(),
        None,
        Visibility::Public,
        &mut ctx.diagnostics,
    );
    ctx.symbol_table.symbols[from_residual_sym.0 as usize].provider_id = Some(core_provider);

    // Foreign Enum: Option
    let option_sym = ctx.symbol_table.declare_symbol(
        "Option".to_string(),
        SymbolKind::Enum,
        core_scope,
        Span::default(),
        None,
        Visibility::Public,
        &mut ctx.diagnostics,
    );
    ctx.symbol_table.symbols[option_sym.0 as usize].provider_id = Some(core_provider);
    let option_ty = ctx.types.intern(SemanticType::Enum(option_sym, Vec::new(), Vec::new()));
    ctx.tables.symbol_types.insert(option_sym, option_ty);

    // Foreign Enum: Result
    let result_sym = ctx.symbol_table.declare_symbol(
        "Result".to_string(),
        SymbolKind::Enum,
        core_scope,
        Span::default(),
        None,
        Visibility::Public,
        &mut ctx.diagnostics,
    );
    ctx.symbol_table.symbols[result_sym.0 as usize].provider_id = Some(core_provider);
    let result_ty = ctx.types.intern(SemanticType::Enum(result_sym, Vec::new(), Vec::new()));
    ctx.tables.symbol_types.insert(result_sym, result_ty);

    (core_scope, clone_sym, from_residual_sym, option_sym, result_sym)
}

// Case 1: Local trait, Foreign type -> PASS
#[test]
fn test_case_01_local_trait_foreign_type_pass() {
    let source = r#"
        trait MyDisplay {
            fn display(self: &Self) -> i32;
        }
        impl MyDisplay for core::Option {
            fn display(self: &Self) -> i32 { return 42; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(success, "Expected pass for local trait on foreign type, got: {:?}", ctx.diagnostics);
}

// Case 2: Foreign trait, Local type -> PASS
#[test]
fn test_case_02_foreign_trait_local_type_pass() {
    let source = r#"
        struct MyStruct {
            val: i32,
        }
        impl core::Clone for MyStruct {
            fn clone(self: &Self) -> MyStruct { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(success, "Expected pass for foreign trait on local type, got: {:?}", ctx.diagnostics);
}

// Case 3: Foreign trait, Local generic struct -> PASS
#[test]
fn test_case_03_foreign_trait_local_generic_struct_pass() {
    let source = r#"
        struct MyBox<T> {
            val: T,
        }
        impl<T> core::Clone for MyBox<T> {
            fn clone(self: &Self) -> MyBox<T> { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(success, "Expected pass for foreign trait on local generic struct, got: {:?}", ctx.diagnostics);
}

// Case 4: Inherent impl on local type -> PASS
#[test]
fn test_case_04_local_inherent_impl_pass() {
    let source = r#"
        struct MyStruct {
            val: i32,
        }
        impl MyStruct {
            fn get_val(self: &Self) -> i32 { return self.val; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected pass for local inherent impl, got: {:?}", ctx.diagnostics);
}

// Case 5: Foreign trait, Foreign type -> FAIL (E_ORPHAN_IMPL)
#[test]
fn test_case_05_orphan_foreign_trait_foreign_type_fail() {
    let source = r#"
        impl core::Clone for core::Option {
            fn clone(self: &Self) -> core::Option { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(!success, "Expected failure for foreign trait on foreign type");
    let has_orphan_err = ctx.diagnostics.iter().any(|d| d.message.contains("E_ORPHAN_IMPL"));
    assert!(has_orphan_err, "Expected E_ORPHAN_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 6: Foreign trait, Primitive type -> FAIL (E_ORPHAN_IMPL)
#[test]
fn test_case_06_orphan_foreign_trait_primitive_fail() {
    let source = r#"
        impl core::Clone for i32 {
            fn clone(self: &Self) -> i32 { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(!success, "Expected failure for foreign trait on primitive type");
    let has_orphan_err = ctx.diagnostics.iter().any(|d| d.message.contains("E_ORPHAN_IMPL"));
    assert!(has_orphan_err, "Expected E_ORPHAN_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 7: Inherent impl on Foreign type -> FAIL (E_ORPHAN_IMPL)
#[test]
fn test_case_07_orphan_inherent_foreign_type_fail() {
    let source = r#"
        impl core::Option {
            fn foo(self: &Self) -> i32 { return 0; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(!success, "Expected failure for inherent impl on foreign type");
    let has_orphan_err = ctx.diagnostics.iter().any(|d| d.message.contains("E_ORPHAN_IMPL"));
    assert!(has_orphan_err, "Expected E_ORPHAN_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 8: Generic args don't confer locality -> FAIL (E_ORPHAN_IMPL)
#[test]
fn test_case_08_orphan_generic_args_no_locality_fail() {
    let source = r#"
        struct LocalType {
            x: i32,
        }
        impl core::Clone for core::Result {
            fn clone(self: &Self) -> core::Result { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_setup(source, |ctx| {
        setup_core_provider(ctx);
    });
    assert!(!success, "Expected failure when nominal head is foreign despite local contents");
    let has_orphan_err = ctx.diagnostics.iter().any(|d| d.message.contains("E_ORPHAN_IMPL"));
    assert!(has_orphan_err, "Expected E_ORPHAN_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 9: Duplicate exact impls -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_09_coherence_duplicate_exact_fail() {
    let source = r#"
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S {
            x: i32,
        }
        impl Foo for S {
            fn f(self: &Self) -> i32 { return 1; }
        }
        impl Foo for S {
            fn f(self: &Self) -> i32 { return 2; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure for duplicate exact impls");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 10: Local impl vs Imported impl collision -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_10_coherence_local_vs_imported_duplicate_fail() {
    let source = r#"
        struct S {
            x: i32,
        }
        impl core::Clone for S {
            fn clone(self: &Self) -> S { return *self; }
        }
    "#;
    let (ctx, success) = run_semantic_with_hook(
        source,
        |ctx| {
            setup_core_provider(ctx);
        },
        |ctx| {
            let core_scope = *ctx.external_module_scopes.get("core").expect("core scope");
            let clone_sym = ctx.symbol_table.lookup("Clone", core_scope).expect("Clone sym");
            // Resolver has resolved S in local scope.
            let s_sym = ctx.symbol_table.lookup("S", ScopeId(0)).expect("S should be resolved");
            let s_ty = ctx.tables.symbol_types.get(&s_sym).copied().unwrap_or_else(|| {
                ctx.types.intern(SemanticType::Struct(s_sym, Vec::new(), Vec::new()))
            });
            // Simulate that an imported provider already implemented Clone for S
            ctx.tables.trait_impl_entries.push(mellis_semantic::semantic_tables::TraitImplEntry {
                decl_id: None,
                trait_id: clone_sym,
                self_type: s_ty,
                generic_params: Vec::new(),
            });
        },
    );
    assert!(!success, "Expected failure when local impl collides with imported impl");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 11: Cross-imported providers collision -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_11_coherence_cross_imported_providers_collision_fail() {
    let mut ctx = SemanticContext::new();
    let (_, clone_sym, _, option_sym, _) = setup_core_provider(&mut ctx);

    // Provider A implemented Clone for Option
    let opt_ty = ctx.types.intern(SemanticType::Enum(option_sym, Vec::new(), Vec::new()));
    ctx.tables.trait_impl_entries.push(mellis_semantic::semantic_tables::TraitImplEntry {
        decl_id: None,
        trait_id: clone_sym,
        self_type: opt_ty,
        generic_params: Vec::new(),
    });

    // Provider B attempts to also inject Clone for Option
    let res = ctx.check_impl_coherence(clone_sym, opt_ty, &[], Span::default());
    assert!(res.is_err(), "Expected coherence check to reject overlapping cross-imported impl");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 12: Primitive duplicate impls -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_12_coherence_primitive_duplicate_fail() {
    let source = r#"
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        impl Foo for i32 {
            fn f(self: &Self) -> i32 { return 1; }
        }
        impl Foo for i32 {
            fn f(self: &Self) -> i32 { return 2; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure for duplicate impls on primitive type");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 13: Generic vs concrete overlap -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_13_coherence_generic_vs_concrete_overlap_fail() {
    let source = r#"
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct Box<T> {
            val: T,
        }
        impl<T> Foo for Box<T> {
            fn f(self: &Self) -> i32 { return 1; }
        }
        impl Foo for Box<i32> {
            fn f(self: &Self) -> i32 { return 2; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure for generic vs concrete overlap");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 14: Two generics overlap -> FAIL (E_CONFLICTING_TRAIT_IMPL)
#[test]
fn test_case_14_coherence_two_generics_overlap_fail() {
    let source = r#"
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S<T> {
            val: T,
        }
        impl<T> Foo for S<T> {
            fn f(self: &Self) -> i32 { return 1; }
        }
        impl<U> Foo for S<U> {
            fn f(self: &Self) -> i32 { return 2; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure for two generic overlapping impls");
    let has_conflict = ctx.diagnostics.iter().any(|d| {
        d.message.contains("E_CONFLICTING_TRAIT_IMPL") || d.message.contains("conflicting implementations for trait")
    });
    assert!(has_conflict, "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}", ctx.diagnostics);
}

// Case 15: Non-overlapping generic instantiations -> PASS
#[test]
fn test_case_15_coherence_non_overlapping_generics_pass() {
    let source = r#"
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S<T> {
            val: T,
        }
        impl Foo for S<i32> {
            fn f(self: &Self) -> i32 { return 1; }
        }
        impl Foo for S<f64> {
            fn f(self: &Self) -> i32 { return 2; }
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected pass for non-overlapping generic instantiations, got: {:?}", ctx.diagnostics);
}
