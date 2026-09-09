use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::SemanticContext;

fn run_semantic(source: &str) -> (SemanticContext, bool) {
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
    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&items);

    if !ctx.diagnostics.is_empty() {
        return (ctx, false);
    }

    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&items);
    let success = ctx.diagnostics.is_empty();
    (ctx, success)
}

// 1. Concrete Normalization: `MyIter::Item` normalizes to `i32`
#[test]
fn test_concrete_normalization() {
    let source = r#"
        trait Iterator {
            type Item;
            fn next(self: &rw Self) -> Self::Item;
        }
        struct MyIter {
            val: i32,
        }
        impl Iterator for MyIter {
            type Item = i32;
            fn next(self: &rw Self) -> Self::Item {
                return self.val;
            }
        }
        fn test_call(it: &rw MyIter) -> i32 {
            return it.next();
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected successful normalization, diagnostics: {:?}", ctx.diagnostics);
}

// 2. Generic Symbolic Projection: `T::Item` remains symbolic during analysis
#[test]
fn test_generic_symbolic_projection() {
    let source = r#"
        trait Iterator {
            type Item;
            fn next(self: &rw Self) -> Self::Item;
        }
        fn get_next<T: Iterator>(it: &rw T) -> T::Item {
            return it.next();
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected successful generic symbolic projection, diagnostics: {:?}", ctx.diagnostics);
}

// 3. Ambiguity Rejection: `E_AMBIGUOUS_ASSOCIATED_TYPE`
#[test]
fn test_ambiguity_rejection() {
    let source = r#"
        trait TraitA {
            type Item;
        }
        trait TraitB {
            type Item;
        }
        fn f<T: TraitA + TraitB>(x: T) -> T::Item {
            return 0;
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure due to ambiguous associated type");
    let has_diag = ctx.diagnostics.iter().any(|d| d.message.contains("E_AMBIGUOUS_ASSOCIATED_TYPE"));
    assert!(has_diag, "Expected E_AMBIGUOUS_ASSOCIATED_TYPE, diagnostics: {:?}", ctx.diagnostics);
}

// 4. Missing Impl Rejection: `E_UNRESOLVED_TRAIT_IMPL`
#[test]
fn test_missing_impl_rejection() {
    let source = r#"
        trait TraitA {
            type Item;
        }
        struct Foo {
            x: i32,
        }
        fn f(x: Foo) -> Foo::Item {
            return 0;
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure due to missing trait impl");
    let has_diag = ctx.diagnostics.iter().any(|d| d.message.contains("E_UNRESOLVED_TRAIT_IMPL"));
    assert!(has_diag, "Expected E_UNRESOLVED_TRAIT_IMPL, diagnostics: {:?}", ctx.diagnostics);
}

// 5. Missing Definition Rejection: `E_NO_ASSOCIATED_TYPE`
#[test]
fn test_missing_definition_rejection() {
    let source = r#"
        trait TraitA {
            type Item;
        }
        struct Foo {
            x: i32,
        }
        impl TraitA for Foo {
            // omits type Item
        }
        fn f(x: Foo) -> Foo::Item {
            return 0;
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected failure due to missing associated type in impl");
    let has_diag = ctx.diagnostics.iter().any(|d| d.message.contains("E_NO_ASSOCIATED_TYPE"));
    assert!(has_diag, "Expected E_NO_ASSOCIATED_TYPE, diagnostics: {:?}", ctx.diagnostics);
}

// 6. Monomorphization Barrier: Unresolved projection fails `is_monomorphic`
#[test]
fn test_monomorphization_barrier_projection() {
    let mut ctx = SemanticContext::new();
    let span = mellis_common::Span::new(FileId(0), 0, 0);
    let mut diags = Vec::new();
    let sym_self = ctx.symbol_table.declare_symbol("T".to_string(), mellis_semantic::SymbolKind::TypeParam, mellis_semantic::symbol::ScopeId(0), span, None, mellis_ast::Visibility::Public, &mut diags);
    let t_id = ctx.types.intern(mellis_semantic::SemanticType::GenericParam(sym_self));
    let trait_sym = ctx.symbol_table.declare_symbol("Iterator".to_string(), mellis_semantic::SymbolKind::Trait, mellis_semantic::symbol::ScopeId(0), span, None, mellis_ast::Visibility::Public, &mut diags);
    let assoc_sym = ctx.symbol_table.declare_symbol("Item".to_string(), mellis_semantic::SymbolKind::AssociatedType, mellis_semantic::symbol::ScopeId(0), span, None, mellis_ast::Visibility::Public, &mut diags);

    let proj_ty = ctx.types.intern(mellis_semantic::SemanticType::Projection {
        self_type: t_id,
        trait_id: trait_sym,
        assoc_type: assoc_sym,
    });

    assert!(!ctx.types.is_monomorphic(proj_ty), "Unresolved projection must not be monomorphic!");
    assert!(ctx.types.contains_projection(proj_ty), "contains_projection must be true for projection type");
}

// 7. Poison Containment: Error type fails `is_monomorphic`
#[test]
fn test_poison_containment() {
    let mut ctx = SemanticContext::new();
    let err_ty = ctx.types.intern(mellis_semantic::SemanticType::Error);

    assert!(!ctx.types.is_monomorphic(err_ty), "SemanticType::Error must fail is_monomorphic check!");

    // Composite types containing error must also fail
    let span = mellis_common::Span::new(FileId(0), 0, 0);
    let mut diags = Vec::new();
    let struct_sym = ctx.symbol_table.declare_symbol("Foo".to_string(), mellis_semantic::SymbolKind::Struct, mellis_semantic::symbol::ScopeId(0), span, None, mellis_ast::Visibility::Public, &mut diags);
    let poisoned_struct = ctx.types.intern(mellis_semantic::SemanticType::Struct(struct_sym, vec![err_ty], vec![]));
    assert!(!ctx.types.is_monomorphic(poisoned_struct), "Poisoned struct must fail is_monomorphic check!");
}

// 8. Projection Cycle Detection: `E_ASSOC_TYPE_CYCLE`
#[test]
fn test_projection_cycle_detection() {
    let source = r#"
        trait Loop {
            type Item;
        }
        struct Cycler {}
        impl Loop for Cycler {
            type Item = Cycler::Item;
        }
        fn trigger(x: Cycler::Item) {}
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(!success, "Expected cycle detection error");
    let has_diag = ctx.diagnostics.iter().any(|d| d.message.contains("E_ASSOC_TYPE_CYCLE"));
    assert!(has_diag, "Expected E_ASSOC_TYPE_CYCLE, diagnostics: {:?}", ctx.diagnostics);
}

// 9. Associated Type Equality Constraint: `T: Iterator<Item = i32>`
#[test]
fn test_associated_equality_constraint() {
    let source = r#"
        trait Iterator {
            type Item;
            fn next(self: &rw Self) -> Self::Item;
        }
        struct IntIter {
            val: i32,
        }
        impl Iterator for IntIter {
            type Item = i32;
            fn next(self: &rw Self) -> Self::Item {
                return self.val;
            }
        }
        fn consume_int_iter<T: Iterator<Item = i32> >(it: &rw T) -> i32 {
            return it.next() + 1;
        }
        fn test_pass(it: &rw IntIter) -> i32 {
            return consume_int_iter(it);
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected successful associated equality check, diagnostics: {:?}", ctx.diagnostics);

    // Negative case: passing BoolIter to consume_int_iter
    let bad_source = r#"
        trait Iterator {
            type Item;
            fn next(self: &rw Self) -> Self::Item;
        }
        struct BoolIter {
            val: bool,
        }
        impl Iterator for BoolIter {
            type Item = bool;
            fn next(self: &rw Self) -> Self::Item {
                return self.val;
            }
        }
        fn consume_int_iter<T: Iterator<Item = i32> >(it: &rw T) -> i32 {
            return 0;
        }
        fn test_fail(it: &rw BoolIter) -> i32 {
            return consume_int_iter(it);
        }
    "#;
    let (ctx_bad, success_bad) = run_semantic(bad_source);
    assert!(!success_bad, "Expected failure when passing BoolIter where Item = i32 was expected");
    let has_diag = ctx_bad.diagnostics.iter().any(|d| d.message.contains("E_ASSOCIATED_TYPE_MISMATCH"));
    assert!(has_diag, "Expected E_ASSOCIATED_TYPE_MISMATCH, diagnostics: {:?}", ctx_bad.diagnostics);
}

// 10. Deep Generic Substitution: nested structs, tuples, and references
#[test]
fn test_generic_substitution_deep() {
    let source = r#"
        trait Container {
            type Elem;
        }
        struct Wrapper<T> {
            val: T,
        }
        impl<T> Container for Wrapper<T> {
            type Elem = T;
        }
        fn extract_nested(w: &Wrapper<(i32, bool)>) -> Wrapper<(i32, bool)>::Elem {
            return w.val;
        }
    "#;
    let (ctx, success) = run_semantic(source);
    assert!(success, "Expected successful deep generic projection normalization, diagnostics: {:?}", ctx.diagnostics);
}

