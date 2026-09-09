use mellis_ast::{AstArena, Decl, Expr, Item, Stmt};
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;

fn parse(input: &str) -> (Result<Vec<Item>, ()>, AstArena, Vec<mellis_common::Diagnostic>) {
    let mut arena = AstArena::default();
    let file_id = FileId(0);
    let lexer = Lexer::new(input, file_id);
    let tokens: Vec<mellis_lexer::Token> = lexer.collect();
    let mut parser = Parser::from_tokens(tokens, input, None, &mut arena, file_id);
    let result = parser.parse_file();
    let diags = parser.diagnostics;
    (result, arena, diags)
}

#[test]
fn test_parse_const_declaration() {
    let src = "const MAX_BUFFER: usize = 1024;";
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Var { name, is_const, is_mutable, type_annot, initializer, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(&src[name.start as usize..name.end as usize], "MAX_BUFFER");
            assert!(*is_const);
            assert!(!*is_mutable);
            assert!(type_annot.is_some());
            assert!(initializer.is_some());
        } else {
            panic!("Expected Decl::Var with is_const");
        }
    }
}

#[test]
fn test_parse_comptime_block_expr() {
    let src = r#"
fn compute() -> i32 {
    dec x = comptime {
        dec a = 10;
        dec b = 20;
        a + b
    };
    x
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(fn_id) = items[0] {
        if let Decl::Function { body: Some(body_stmt), .. } = &arena.decls[fn_id.0 as usize] {
            if let Stmt::Block { body, .. } = &arena.stmts[body_stmt.0 as usize] {
                if let Item::Decl(var_id) = body[0] {
                    if let Decl::Var { initializer: Some(init_expr), .. } = &arena.decls[var_id.0 as usize] {
                        if let Expr::Comptime { body: comptime_body } = &arena.exprs[init_expr.0 as usize] {
                            if let Stmt::Block { body: ct_body, tail_expr: ct_tail } = &arena.stmts[comptime_body.0 as usize] {
                                assert_eq!(ct_body.len(), 2);
                                assert!(ct_tail.is_some());
                            } else {
                                panic!("Expected Stmt::Block inside Comptime");
                            }
                        } else {
                            panic!("Expected Expr::Comptime");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_parse_const_comptime_combination() {
    let src = "const FACT_5 = comptime { 120 };";
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Var { is_const, initializer: Some(init_id), .. } = &arena.decls[decl_id.0 as usize] {
            assert!(*is_const);
            assert!(matches!(&arena.exprs[init_id.0 as usize], Expr::Comptime { .. }));
        }
    }
}
