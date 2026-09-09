use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::macro_engine::MacroEngine;
use mellis_semantic::resolver::Resolver;
use mellis_semantic::typechecker::TypeChecker;
use mellis_semantic::SemanticContext;

fn check_code(source: &str) -> Result<mellis_ast::AstArena, Vec<mellis_common::Diagnostic>> {
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
fn test_hygiene_local_variable_no_collision() {
    // Macro declares local `tmp`, caller also declares `tmp` in the same function.
    // Without hygiene, this would trigger "Duplicate definition of symbol `tmp` in the same scope".
    let code = r#"
    macro set_internal {
        (@val: expr) => {
            dec tmp = @val;
        }
    }

    fn main() -> i32 {
        dec tmp: i32 = 10;
        set_internal!(20);
        return tmp;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed hygiene local variable anti-shadowing: {:?}", res.err());
}

#[test]
fn test_hygiene_same_macro_invoked_twice() {
    // Same macro invoked twice in the same scope must assign distinct expansion contexts,
    // preventing collision between the two generated `tmp` variables.
    let code = r#"
    macro declare_internal {
        (@val: expr) => {
            dec tmp = @val;
        }
    }

    fn main() -> i32 {
        declare_internal!(1);
        declare_internal!(2);
        return 0;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed repeated invocation hygiene: {:?}", res.err());
}

#[test]
fn test_hygiene_nested_macro_expansion() {
    // Nested expansion:
    // outer expansion creates context #1, inner expansion creates context #2.
    // tmp#2 must not collide with outer macro or caller identifiers.
    let code = r#"
    macro inner {
        () => {
            dec tmp = 1;
        }
    }

    macro outer {
        () => {
            dec outer_var = 10;
            inner!();
        }
    }

    fn main() -> i32 {
        dec tmp = 42;
        outer!();
        return tmp;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed nested macro expansion hygiene: {:?}", res.err());
}

#[test]
fn test_hygiene_argument_captures_call_site_context() {
    // An identifier passed from the caller (@input) must resolve to caller's `x`,
    // NOT the macro's internal `x`!
    let code = r#"
    macro add_with_internal {
        (@input: expr, @target: ident) => {
            dec x: i32 = 100;
            @target = @input + x;
        }
    }

    fn main() -> i32 {
        dec x: i32 = 5;
        dec rw res: i32 = 0;
        add_with_internal!(x * 2, res);
        return res;
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed referential transparency / call-site argument capture: {:?}", res.err());
}

#[test]
fn test_hygiene_transparent_global_item_lookup() {
    // Macro template can reference global functions transparently through root fallback
    let code = r#"
    fn helper_calc(a: i32) -> i32 {
        return a + 10;
    }

    macro call_helper {
        (@x: expr) => {
            helper_calc(@x)
        }
    }

    fn main() -> i32 {
        return call_helper!(5);
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed transparent fallback for global function lookup: {:?}", res.err());
}

#[test]
fn test_hygiene_definition_site_resolution() {
    let code = r#"
    fn helper() -> i32 { return 1; }

    macro foo {
        (@target: ident) => {
            @target = helper();
        }
    }

    fn caller() -> i32 {
        dec rw x: i32 = 0;
        foo!(x);
        return x;
    }

    fn main() -> i32 {
        return caller();
    }
    "#;
    let res = check_code(code);
    assert!(res.is_ok(), "Failed definition-site resolution: {:?}", res.err());
}
