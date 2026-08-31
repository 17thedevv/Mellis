use mellis_ast::{
    AstArena, Decl, Expr, FragmentKind, Item, MacroDelimiter, MatcherElement, Stmt,
    TranscriberElement,
};
use mellis_common::ids::FileId;
use mellis_lexer::Lexer;
use mellis_parser::Parser;

fn parse(input: &str) -> (Result<Vec<Item>, ()>, AstArena, Vec<mellis_common::Diagnostic>) {
    let mut arena = AstArena::default();
    let file_id = FileId(0);
    let lexer = Lexer::new(input, file_id);
    let tokens: Vec<mellis_lexer::Token> = lexer.collect();
    let mut parser = Parser::from_tokens(tokens, input, &mut arena, file_id);
    let result = parser.parse_file();
    let diags = parser.diagnostics;
    (result, arena, diags)
}

#[test]
fn test_simple_macro() {
    let src = r#"
macro foo {
    (@x: expr) => {
        @x * 2
    }
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Macro { name, rules, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(&src[name.start as usize..name.end as usize], "foo");
            assert_eq!(rules.len(), 1);

            let rule = &rules[0];
            assert_eq!(rule.pattern.delimiter, MacroDelimiter::Paren);
            assert_eq!(rule.pattern.elements.len(), 1);
            match &rule.pattern.elements[0] {
                MatcherElement::MetaVar { name, fragment, .. } => {
                    assert_eq!(&src[name.start as usize..name.end as usize], "x");
                    assert_eq!(*fragment, FragmentKind::Expr);
                }
                other => panic!("Expected MetaVar, got {:?}", other),
            }

            assert_eq!(rule.transcriber.delimiter, MacroDelimiter::Brace);
            assert_eq!(rule.transcriber.elements.len(), 3);
            match &rule.transcriber.elements[0] {
                TranscriberElement::MetaVar { name, .. } => {
                    assert_eq!(&src[name.start as usize..name.end as usize], "x");
                }
                other => panic!("Expected MetaVar, got {:?}", other),
            }
        } else {
            panic!("Expected Decl::Macro");
        }
    } else {
        panic!("Expected Item::Decl");
    }
}

#[test]
fn test_macro_with_multiple_fragments() {
    let src = r#"
macro add_fields {
    (@x: expr, @y: expr) => {
        @x + @y
    }
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Macro { rules, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(rules.len(), 1);
            let rule = &rules[0];
            // Elements: MetaVar(@x), Leaf(,), MetaVar(@y)
            assert_eq!(rule.pattern.elements.len(), 3);
            match &rule.pattern.elements[0] {
                MatcherElement::MetaVar { fragment, .. } => assert_eq!(*fragment, FragmentKind::Expr),
                _ => panic!("Expected MetaVar"),
            }
            match &rule.pattern.elements[1] {
                MatcherElement::Leaf { token } => assert_eq!(token.kind, mellis_lexer::TokenKind::Comma),
                _ => panic!("Expected Leaf Comma"),
            }
            match &rule.pattern.elements[2] {
                MatcherElement::MetaVar { fragment, .. } => assert_eq!(*fragment, FragmentKind::Expr),
                _ => panic!("Expected MetaVar"),
            }
        } else {
            panic!("Expected Decl::Macro");
        }
    }
}

#[test]
fn test_macro_with_multiple_rules() {
    let src = r#"
macro compute {
    (@x: expr) => {
        @x
    }
    (@x: expr, @y: expr) => {
        @x + @y
    }
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Macro { rules, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(rules.len(), 2);
            assert_eq!(rules[0].pattern.elements.len(), 1);
            assert_eq!(rules[1].pattern.elements.len(), 3);
        } else {
            panic!("Expected Decl::Macro");
        }
    }
}

#[test]
fn test_all_fragment_kinds() {
    let fragments = [
        ("expr", FragmentKind::Expr),
        ("ident", FragmentKind::Ident),
        ("ty", FragmentKind::Ty),
        ("stmt", FragmentKind::Stmt),
        ("block", FragmentKind::Block),
        ("item", FragmentKind::Item),
    ];

    for (frag_str, expected_kind) in fragments {
        let src = format!("macro m {{ (@val: {}) => {{ @val }} }}", frag_str);
        let (res, arena, diags) = parse(&src);
        assert!(diags.is_empty(), "Failed for {}: {:?}", frag_str, diags);
        let items = res.expect("Failed to parse");
        if let Item::Decl(decl_id) = items[0] {
            if let Decl::Macro { rules, .. } = &arena.decls[decl_id.0 as usize] {
                match &rules[0].pattern.elements[0] {
                    MatcherElement::MetaVar { fragment, .. } => {
                        assert_eq!(*fragment, expected_kind, "Kind mismatch for {}", frag_str);
                    }
                    _ => panic!("Expected MetaVar"),
                }
            }
        }
    }
}

#[test]
fn test_macro_invocations_paren_bracket_brace() {
    let src = r#"
fn test_calls() {
    dec a = foo!(1, 2);
    dec b = bar![3, 4];
    dec c = baz!{5, 6};
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Function { body: Some(body_id), .. } = &arena.decls[decl_id.0 as usize] {
            if let Stmt::Block { body, .. } = &arena.stmts[body_id.0 as usize] {
                assert_eq!(body.len(), 3);
                let delimiters = [
                    MacroDelimiter::Paren,
                    MacroDelimiter::Bracket,
                    MacroDelimiter::Brace,
                ];

                for (idx, expected_del) in delimiters.iter().enumerate() {
                    if let Item::Decl(var_id) = body[idx] {
                        if let Decl::Var { initializer: Some(init_id), .. } = &arena.decls[var_id.0 as usize] {
                            if let Expr::MacroCall { delimiter, args, .. } = &arena.exprs[init_id.0 as usize] {
                                assert_eq!(delimiter, expected_del);
                                assert!(!args.is_empty());
                            } else {
                                panic!("Expected Expr::MacroCall");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_malformed_macro_definition_missing_name() {
    let src = "macro { (@x: expr) => { @x } }";
    let (_res, _arena, diags) = parse(src);
    assert!(!diags.is_empty(), "Expected error for missing macro name");
    assert!(diags.iter().any(|d| d.message.contains("Expected macro name")));
}

#[test]
fn test_malformed_macro_definition_missing_arrow() {
    let src = "macro foo { (@x: expr) { @x } }";
    let (_res, _arena, diags) = parse(src);
    assert!(!diags.is_empty(), "Expected error for missing '=>'");
    assert!(diags.iter().any(|d| d.message.contains("Expected '=>'")));
}

#[test]
fn test_malformed_fragment_specifier() {
    let src = "macro foo { (@x: invalid_frag) => { @x } }";
    let (_res, _arena, diags) = parse(src);
    assert!(!diags.is_empty(), "Expected error for unknown fragment specifier");
    assert!(diags.iter().any(|d| d.message.contains("Unknown macro fragment specifier")));
}

#[test]
fn test_malformed_macro_invocation() {
    let src = r#"
fn main() {
    foo! 42;
}
"#;
    let (_res, _arena, diags) = parse(src);
    assert!(!diags.is_empty(), "Expected error for invalid macro invocation delimiter");
    assert!(diags.iter().any(|d| d.message.contains("Expected '(', '[', or '{' after macro '!'")));
}

#[test]
fn test_nested_delimiters_in_macro() {
    let src = r#"
macro complex_rule {
    ((@a: expr, @b: expr), [@c: expr]) => {
        (@a + @b) * @c
    }
}
"#;
    let (res, arena, diags) = parse(src);
    assert!(diags.is_empty(), "Unexpected diagnostics: {:?}", diags);
    let items = res.expect("Failed to parse");
    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Macro { rules, .. } = &arena.decls[decl_id.0 as usize] {
            let rule = &rules[0];
            // Pattern has 3 top elements: Group(Paren), Leaf(Comma), Group(Bracket)
            assert_eq!(rule.pattern.elements.len(), 3);
            match &rule.pattern.elements[0] {
                MatcherElement::Group { delimiter, elements, .. } => {
                    assert_eq!(*delimiter, MacroDelimiter::Paren);
                    assert_eq!(elements.len(), 3); // MetaVar, Leaf, MetaVar
                }
                _ => panic!("Expected nested Group"),
            }
            match &rule.pattern.elements[2] {
                MatcherElement::Group { delimiter, elements, .. } => {
                    assert_eq!(*delimiter, MacroDelimiter::Bracket);
                    assert_eq!(elements.len(), 1); // MetaVar
                }
                _ => panic!("Expected nested Group"),
            }

            // Transcriber has Group(Paren), Leaf(*), MetaVar
            match &rule.transcriber.elements[0] {
                TranscriberElement::Group { delimiter, elements, .. } => {
                    assert_eq!(*delimiter, MacroDelimiter::Paren);
                    assert_eq!(elements.len(), 3);
                }
                _ => panic!("Expected nested Group"),
            }
        }
    }
}
