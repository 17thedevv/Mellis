use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::macro_engine::MacroEngine;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::SemanticContext;

fn expand_code(source: &str) -> Result<mellis_ast::AstArena, Vec<mellis_common::Diagnostic>> {
    let mut source_manager = mellis_common::source::SourceManager::new();
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

    Resolver::new(&mut ctx, &arena, &source_manager).resolve_items(&expanded);
    TypeChecker::new(&mut ctx, &arena, &source_manager).typecheck_items(&expanded);

    if !ctx.diagnostics.is_empty() {
        return Err(ctx.diagnostics);
    }

    Ok(arena)
}

#[test]
fn test_macro_tree_matching_and_backtracking() {
    let code = r#"
    macro test_backtrack {
        (@x: ident, @y: ident) => { 1 }
        (@x: expr, @y: expr) => { 2 }
    }

    fn main() -> i32 {
        dec a: i32 = test_backtrack!(1 + 2, 3 + 4);
        return a;
    }
    "#;
    let res = expand_code(code);
    assert!(res.is_ok(), "Failed to expand macro with backtracking: {:?}", res.err());
}

#[test]
fn test_macro_nested_delimiter_groups() {
    let code = r#"
    macro nested_group {
        ([@x: expr], [@y: expr]) => {
            @x + @y
        }
    }

    fn main() -> i32 {
        dec res: i32 = nested_group!([10], [20]);
        return res;
    }
    "#;
    let res = expand_code(code);
    assert!(res.is_ok(), "Failed to match nested delimiter groups: {:?}", res.err());
}

#[test]
fn test_macro_recursion_limit_guard() {
    let code = r#"
    macro infinite_loop {
        (@x: expr) => {
            infinite_loop!(@x)
        }
    }

    fn main() -> i32 {
        dec res = infinite_loop!(1);
        return res;
    }
    "#;
    let res = expand_code(code);
    assert!(res.is_err(), "Expected recursion limit diagnostic");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("recursion limit reached")),
        "Expected recursion limit message, got: {:?}",
        errs
    );
}

#[test]
fn test_macro_fragment_expr_not_swallowing_comma() {
    let code = r#"
    macro pair {
        (@x: expr, @y: expr) => {
            @x * 10 + @y
        }
    }

    fn main() -> i32 {
        dec a = pair!(1 + 2, 3 + 4);
        return a;
    }
    "#;
    let res = expand_code(code);
    assert!(res.is_ok(), "Expr fragment swallowed comma or failed parsing: {:?}", res.err());
}
