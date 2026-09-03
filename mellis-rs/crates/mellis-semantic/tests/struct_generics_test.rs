use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::SemanticContext;

#[test]
fn test_normal_struct_generics() {
    let source = "struct Container<T> { val: T }";
    let file_id = FileId(0);
    let mut arena = AstArena::new();
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().expect("parse_file failed");

    let mut ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut ctx, &arena, source);
    resolver.resolve_items(&items);
    
    assert!(ctx.diagnostics.is_empty(), "Resolver Errors: {:#?}", ctx.diagnostics);
    
    TypeChecker::new(&mut ctx, &arena, source).typecheck_items(&items);

    assert!(ctx.diagnostics.is_empty(), "TypeChecker Errors: {:#?}", ctx.diagnostics);
}
