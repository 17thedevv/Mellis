use luna_ast::{AstArena, Decl, Item, Visibility};
use luna_common::ids::FileId;
use luna_lexer::Lexer;
use luna_parser::Parser;

fn parse(input: &str) -> (Result<Vec<Item>, ()>, AstArena, Vec<luna_common::Diagnostic>) {
    let mut arena = AstArena::default();
    let file_id = FileId(0);
    let lexer = Lexer::new(input, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let result = parser.parse_file();
    let diags = parser.diagnostics;
    (result, arena, diags)
}

#[test]
fn test_struct_field_visibility_parsing() {
    let input = r#"
        export struct User {
            export name: str,
            password: str,
        }
    "#;
    let (result, arena, diagnostics) = parse(input);
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got {:?}", diagnostics);
    let items = result.unwrap();
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = &items[0] {
        let decl = &arena.decls[decl_id.0 as usize];
        if let Decl::Struct { visibility, fields, .. } = decl {
            assert_eq!(*visibility, Visibility::Public);
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].visibility, Visibility::Public);
            assert_eq!(fields[1].visibility, Visibility::Private);
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}

#[test]
fn test_struct_fields_default_private() {
    let input = r#"
        struct Secret {
            key: i32,
            val: i32,
        }
    "#;
    let (result, arena, diagnostics) = parse(input);
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got {:?}", diagnostics);
    let items = result.unwrap();
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = &items[0] {
        let decl = &arena.decls[decl_id.0 as usize];
        if let Decl::Struct { fields, .. } = decl {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].visibility, Visibility::Private);
            assert_eq!(fields[1].visibility, Visibility::Private);
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}
