use mellis_ast::{AstArena, Item};
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_common::ids::FileId;

fn parse(input: &str) -> (Result<Vec<Item>, ()>, AstArena, Vec<mellis_common::Diagnostic>) {
    let mut arena = AstArena::default();
    let file_id = FileId(0);
    let lexer = Lexer::new(input, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let result = parser.parse_file();
    let diags = parser.diagnostics;
    (result, arena, diags)
}

#[test]
fn test_valid_annotation() {
    let input = r#"
        #[lang("control_flow")]
        enum ControlFlow<B, C> {
            #[lang("break")]
            Break(B),

            #[lang("continue")]
            Continue(C),
        }
    "#;
    let (result, arena, diagnostics) = parse(input);
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got {:?}", diagnostics);
    let items = result.unwrap();
    let enum_item = &items[0];
    if let mellis_ast::Item::Decl(decl_id) = enum_item {
        let decl = &arena.decls[decl_id.0 as usize];
        if let mellis_ast::Decl::Enum { annotations, variants, .. } = decl {
            assert_eq!(annotations.len(), 1);
            
            // Check variant annotations
            let break_var = &variants[0];
            assert_eq!(break_var.annotations.len(), 1);
            
            let continue_var = &variants[1];
            assert_eq!(continue_var.annotations.len(), 1);
        } else {
            panic!("Expected Enum decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}

#[test]
fn test_malformed_annotation_missing_bracket() {
    let input = r#"
        #[lang("control_flow")
        enum Foo {}
    "#;
    let (_, _, diagnostics) = parse(input);
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().any(|d| d.message.contains("Expected ']'")), "Diagnostics were: {:?}", diagnostics);
}

#[test]
fn test_malformed_annotation_standalone_hash() {
    let input = r#"
        #lang("control_flow")
        enum Foo {}
    "#;
    let (_, _, diagnostics) = parse(input);
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().any(|d| d.message.contains("unexpected '#'; expected '#[' for an annotation")));
}

#[test]
fn test_legacy_at_lang_rejected() {
    let input = r#"
        @lang("control_flow")
        enum Foo {}
    "#;
    let (_, _, diagnostics) = parse(input);
    assert!(!diagnostics.is_empty());
    // Should produce some parser diagnostic since @ is unexpected there
}
