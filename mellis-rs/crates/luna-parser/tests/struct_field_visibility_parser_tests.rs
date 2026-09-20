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
        };
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
            // Visibility-02: field without modifier is now Public
            assert_eq!(fields[0].visibility, Visibility::Public);  // explicit export
            assert_eq!(fields[1].visibility, Visibility::Public);  // no modifier → Public
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}

#[test]
fn test_struct_fields_default_public() {
    // Visibility-02: Fields default to Public when no modifier present
    let input = r#"
        struct Point {
            x: i32,
            y: i32,
        };
    "#;
    let (result, arena, diagnostics) = parse(input);
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got {:?}", diagnostics);
    let items = result.unwrap();
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = &items[0] {
        let decl = &arena.decls[decl_id.0 as usize];
        if let Decl::Struct { fields, .. } = decl {
            assert_eq!(fields.len(), 2);
            // Visibility-02: default is now Public
            assert_eq!(fields[0].visibility, Visibility::Public);
            assert_eq!(fields[1].visibility, Visibility::Public);
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}

#[test]
fn test_struct_field_explicit_private() {
    // Visibility-02: explicit private keyword makes field private
    let input = r#"
        export struct BankAccount {
            balance: i64,
            private pin: u32,
        };
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
            assert_eq!(fields[0].visibility, Visibility::Public);  // no modifier
            assert_eq!(fields[1].visibility, Visibility::Private); // explicit private
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}

#[test]
fn test_struct_field_mixed_visibility() {
    // All three visibility forms
    let input = r#"
        export struct User {
            name: str,              // implicit public
            export id: u64,         // explicit public
            private password: str,   // explicit private
        };
    "#;
    let (result, arena, diagnostics) = parse(input);
    assert!(diagnostics.is_empty(), "Expected no diagnostics, got {:?}", diagnostics);
    let items = result.unwrap();
    assert_eq!(items.len(), 1);

    if let Item::Decl(decl_id) = &items[0] {
        let decl = &arena.decls[decl_id.0 as usize];
        if let Decl::Struct { fields, .. } = decl {
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].visibility, Visibility::Public);   // implicit
            assert_eq!(fields[1].visibility, Visibility::Public);   // explicit
            assert_eq!(fields[2].visibility, Visibility::Private);  // explicit
        } else {
            panic!("Expected Struct decl");
        }
    } else {
        panic!("Expected Decl item");
    }
}
