use luna_ast::AstArena;
use luna_common::ids::FileId;
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::resolver::Resolver;
use luna_semantic::typechecker::TypeChecker;
use luna_semantic::SemanticContext;

fn check_code(source: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let mut source_manager = luna_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), source.to_string());
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().expect("parse_file failed");

    let mut ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&items);
    
    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }
    
    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&items);

    if !ctx.diagnostics.is_empty() {
        Err(ctx.diagnostics)
    } else {
        Ok(())
    }
}

#[test]
fn test_single_origin() {
    let source = r#"
        fn foo(x: &i32) -> &i32 life_from(x) {
            return x;
        }
    "#;
    let result = check_code(source);
    assert!(result.is_ok(), "Expected OK, got: {:?}", result.err());
}

#[test]
fn test_multi_origin() {
    let source = r#"
        fn bar(a: &i32, b: &i32) -> &i32 life_from(a | b) {
            if true {
                return a;
            } else {
                return b;
            }
        }
    "#;
    let result = check_code(source);
    assert!(result.is_ok(), "Expected OK, got: {:?}", result.err());
}
