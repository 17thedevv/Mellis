use luna_ast::AstArena;
use luna_common::ids::FileId;
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::macro_engine::MacroEngine;
use luna_semantic::resolver::Resolver;
use luna_semantic::typechecker::TypeChecker;
use luna_semantic::SemanticContext;

fn check_code(source: &str) -> Result<luna_ast::AstArena, Vec<luna_common::Diagnostic>> {
    let mut source_manager = luna_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), source.to_string());
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().expect("parse_file failed");

    let mut ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.register_macros(&items);
    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }

    let mut engine = MacroEngine::new(&mut arena, &source_manager, file_id, &ctx.symbol_table, &ctx.tables);
    let expanded = engine.expand_items(items)?;

    // Verify completion invariant: NO MacroCall node survives anywhere in types or expressions
    for ty in &arena.types {
        if let luna_ast::Type::MacroCall { span, .. } = ty {
            panic!("Invariant violation: unexpanded Type::MacroCall survived expansion pass at {:?}", span);
        }
    }
    for expr in &arena.exprs {
        if let luna_ast::Expr::MacroCall { span, .. } = expr {
            panic!("Invariant violation: unexpanded Expr::MacroCall survived expansion pass at {:?}", span);
        }
    }

    Resolver::new(&mut ctx, &arena, &source_manager).resolve_items(&expanded);
    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&expanded);

    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }

    Ok(arena)
}

#[test]
fn test_macro_in_type_position_pointer() {
    let code = r#"
    macro make_ptr {
        (@t: ty) => { *rw @t }
    }

    fn main() -> i32 {
        dec rw val: i32 = 10;
        dec x: make_ptr!(i32) = &rw val;
        return 0;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed macro in type position: {:?}", res.err());
}

#[test]
fn test_macro_in_function_signatures() {
    let code = r#"
    macro make_ptr {
        (@t: ty) => { *rw @t }
    }

    fn pass_through(x: make_ptr!(i32)) -> make_ptr!(i32) {
        return x;
    }

    fn main() -> i32 {
        dec rw val: i32 = 10;
        dec p: make_ptr!(i32) = &rw val;
        dec res = pass_through(p);
        return 0;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed macro in function signatures: {:?}", res.err());
}

#[test]
fn test_macro_type_alias_generic_substitution() {
    // User requested scenario:
    // type Ptr<T> = *rw T;
    // macro ptr { (@t: ty) => { Ptr<@t> } }
    // fn foo(x: ptr!(i32)) -> ptr!(i32) { return x; }
    let code = r#"
    type Ptr<T> = *rw T;

    macro ptr {
        (@t: ty) => { Ptr<@t> }
    }

    fn foo(x: ptr!(i32)) -> ptr!(i32) {
        return x;
    }

    fn main() -> i32 {
        dec rw val: i32 = 10;
        dec p: ptr!(i32) = &rw val;
        dec out = foo(p);
        return 0;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed macro with type alias and generic substitution: {:?}", res.err());
}

#[test]
fn test_macro_tuple_type() {
    // User requested scenario:
    // macro tuple { (@a: ty, @b: ty) => { (@a, @b) } }
    // dec x: tuple!(i32, bool);
    let code = r#"
    macro tuple {
        (@a: ty, @b: ty) => { (@a, @b) }
    }

    fn main() -> i32 {
        dec x: tuple!(i32, bool) = (42, true);
        return x.0;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed macro tuple type: {:?}", res.err());
}

#[test]
fn test_macro_generating_generic_struct() {
    let code = r#"
    macro make_container {
        (@name: ident) => {
            struct @name<T> {
                val: T,
            }
        }
    }

    make_container!(Container);

    fn main() -> i32 {
        dec c: Container<i32> = Container { val: 42 };
        return c.val;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed macro generating generic struct: {:?}", res.err());
}
