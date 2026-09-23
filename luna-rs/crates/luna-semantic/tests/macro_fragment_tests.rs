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

    resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&expanded);
    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }
    
    let mut tc = TypeChecker::new(&mut ctx, &arena, &source_manager);
    tc.typecheck_items(&expanded);
    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }
    
    Ok(arena)
}

#[test]
fn test_macro_fragment_tt() {
    let source = r#"
        macro capture_tt {
            (@x:tt) => {
                @x
            }
        }
        fn main() {
            capture_tt!(1);
            capture_tt!({ 1; 2; 3; });
            capture_tt!([1, 2, 3]);
        }
    "#;
    let _ = check_code(source).expect("Failed to expand tt fragment");
}

#[test]
fn test_macro_fragment_pat() {
    let source = r#"
        macro capture_pat {
            (@x:pat) => {
                match 1 {
                    @x -> 2,
                    _ -> 3,
                }
            }
        }
        fn main() {
            capture_pat!(1);
        }
    "#;
    let _ = check_code(source).expect("Failed to expand pat fragment");
}

#[test]
fn test_macro_fragment_path() {
    let source = r#"
        macro capture_path {
            (@x:path) => {
                dec val = @x;
            }
        }
        fn main() {
            dec my_var = 5;
            capture_path!(my_var);
        }
    "#;
    let _ = check_code(source).expect("Failed to expand path fragment");
}

#[test]
fn test_macro_fragment_meta() {
    let source = r#"
        macro capture_meta {
            (@x:meta) => {
                @x
                fn inner() {}
            }
        }
        capture_meta!(#[inline]);
    "#;
    let _ = check_code(source).expect("Failed to expand meta fragment");
}

#[test]
fn test_macro_fragment_lifetime() {
    let source = r#"
        macro capture_lifetime {
            (@x:lifetime) => {
                fn check(val: &@x i32) {}
            }
        }
        capture_lifetime!('a);
    "#;
    let _ = check_code(source).expect("Failed to expand lifetime fragment");
}
