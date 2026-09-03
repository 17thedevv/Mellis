use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::SemanticContext;

fn check_code(source: &str) -> Result<(), Vec<mellis_common::Diagnostic>> {
    let file_id = FileId(0);
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().expect("parse_file failed");

    let mut ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut ctx, &arena, source);
    resolver.resolve_items(&items);
    
    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }
    
    TypeChecker::new(&mut ctx, &arena, source).typecheck_items(&items);

    if !ctx.diagnostics.is_empty() {
        Err(ctx.diagnostics)
    } else {
        Ok(())
    }
}

#[test]
fn test_simple_outlives() {
    let source = r#"
        fn foo(a: &i32, b: &i32) where outlives(b, a) {}
    "#;
    let result = check_code(source);
    assert!(result.is_ok(), "Expected OK, got: {:?}", result.err());
}

#[test]
fn test_multi_outlives() {
    let source = r#"
        fn bar(a: &i32, b: &i32, c: &i32) where outlives(b, a), outlives(c, b) {}
    "#;
    let result = check_code(source);
    assert!(result.is_ok(), "Expected OK, got: {:?}", result.err());
}

#[test]
fn test_provenance_and_constraints() {
    let source = r#"
        fn combined(a: &i32, b: &i32) -> &i32 life_from(a | b) where outlives(a, b) {
            return b;
        }
    "#;
    let result = check_code(source);
    assert!(result.is_ok(), "Expected OK, got: {:?}", result.err());
}
