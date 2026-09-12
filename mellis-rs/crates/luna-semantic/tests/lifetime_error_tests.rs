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
    
    for item in &items {
        if let luna_ast::Item::Decl(decl_id) = item {
            let decl = &arena.decls[decl_id.0 as usize];
            if let Err(e) = luna_semantic::lifetime::verify_before_codegen(decl, &ctx, &source_manager, &arena) {
                ctx.diagnostics.push(e.into_diagnostic());
            }
        }
    }

    if !ctx.diagnostics.is_empty() {
        Err(ctx.diagnostics)
    } else {
        Ok(())
    }
}

#[test]
fn test_unresolved_lifetime_in_provenance() {
    let source = r#"
        fn foo(x: &i32) -> &i32 life_from(y) {
            return x;
        }
    "#;
    let result = check_code(source);
    assert!(result.is_err(), "Expected an error");
    let errs = result.unwrap_err();
    println!("Errors: {:#?}", errs);
    assert_eq!(errs.len(), 1);
    assert!(errs[0].message.contains("lifetime 'y' does not refer to any parameter in scope"));
}

#[test]
fn test_unresolved_lifetime_in_constraint() {
    let source = r#"
        fn bar(a: &i32) where outlives(b, a) {}
    "#;
    let result = check_code(source);
    assert!(result.is_err(), "Expected an error");
    let errs = result.unwrap_err();
    println!("Errors: {:#?}", errs);
    assert_eq!(errs.len(), 1);
    assert!(errs[0].message.contains("lifetime 'b' does not refer to any parameter in scope"));
}
