use luna_ast::AstArena;
use luna_common::ids::FileId;
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::resolver::Resolver;
use luna_semantic::typechecker::TypeChecker;
use luna_semantic::SemanticContext;

#[test]
fn test_normal_struct_generics() {
    let source = "struct Container<T> { val: T }";
    let mut source_manager = luna_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), source.to_string());
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().expect("parse_file failed");

    let mut ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&items);
    
    assert!(ctx.diagnostics.is_empty(), "Resolver Errors: {:#?}", ctx.diagnostics);
    
    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&items);

    assert!(ctx.diagnostics.is_empty(), "TypeChecker Errors: {:#?}", ctx.diagnostics);
}
