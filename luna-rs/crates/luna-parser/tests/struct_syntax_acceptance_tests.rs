use luna_ast::{AstArena, Decl, Item};
use luna_common::ids::FileId;
use luna_common::Diagnostic;
use luna_lexer::Lexer;
use luna_parser::Parser;

fn parse(input: &str) -> (Result<Vec<Item>, ()>, AstArena, Vec<Diagnostic>) {
    let mut arena = AstArena::default();
    let file_id = FileId(0);
    let lexer = Lexer::new(input, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let result = parser.parse_file();
    let diags = parser.diagnostics;
    (result, arena, diags)
}

/// STRUCT-SYNTAX-01: struct A {}; -> accepted
#[test]
fn test_struct_syntax_01_empty_struct_with_semicolon() {
    let input = "struct A {};";
    let (result, arena, diags) = parse(input);
    assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    let items = result.expect("parse successful");
    assert_eq!(items.len(), 1);
    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Struct { name, fields, lifetime_contract, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(name.start, 7);
            assert!(fields.is_empty());
            assert!(lifetime_contract.is_none());
        } else {
            panic!("Expected Decl::Struct");
        }
    } else {
        panic!("Expected Item::Decl");
    }
}

/// STRUCT-SYNTAX-02: struct Holder { value: &i32 } requires life(value) >= life(self); -> accepted
#[test]
fn test_struct_syntax_02_struct_with_postfix_lifetime_contract() {
    let input = "struct Holder { value: &i32, } requires life(value) >= life(self);";
    let (result, arena, diags) = parse(input);
    assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    let items = result.expect("parse successful");
    assert_eq!(items.len(), 1);
    if let Item::Decl(decl_id) = items[0] {
        if let Decl::Struct { fields, lifetime_contract, .. } = &arena.decls[decl_id.0 as usize] {
            assert_eq!(fields.len(), 1);
            let contract = lifetime_contract.as_ref().expect("Expected lifetime_contract");
            assert_eq!(contract.constraints.len(), 1);
            assert_eq!(contract.constraints[0].longer.to_string(), "value");
            assert_eq!(contract.constraints[0].shorter.to_string(), "self");
        } else {
            panic!("Expected Decl::Struct");
        }
    } else {
        panic!("Expected Item::Decl");
    }
}

/// STRUCT-SYNTAX-03: struct Holder { value: &i32 }; requires life(value) >= life(self); -> rejected
/// because the first ';' terminates the struct declaration, leaving 'requires' invalid at top level.
#[test]
fn test_struct_syntax_03_semicolon_before_requires_rejected() {
    let input = "struct Holder { value: &i32, }; requires life(value) >= life(self);";
    let (result, _arena, diags) = parse(input);
    assert!(
        result.is_err() || !diags.is_empty(),
        "Expected syntax error when ';' precedes 'requires', but parsing succeeded with no diagnostics"
    );
}

/// STRUCT-SYNTAX-04: struct A {} -> rejected: expected ';' after struct declaration
#[test]
fn test_struct_syntax_04_missing_semicolon_rejected() {
    let input = "struct A {}";
    let (result, _arena, diags) = parse(input);
    assert!(
        result.is_err() || !diags.is_empty(),
        "Expected syntax error for missing semicolon after struct declaration"
    );
    assert!(
        diags.iter().any(|d| d.message.contains("Expected ';'") || d.message.contains("Expected ';' after struct declaration")),
        "Expected error message mentioning missing ';', got: {:?}",
        diags
    );
}

/// STRUCT-SYNTAX-05: struct A {};; -> second ';' rejected normally / treated according to empty-statement policy
#[test]
fn test_struct_syntax_05_double_semicolon_rejected() {
    let input = "struct A {};;";
    let (result, _arena, diags) = parse(input);
    assert!(
        result.is_err() || !diags.is_empty(),
        "Expected syntax error for redundant second semicolon, but got success with no diagnostics"
    );
}
