use luna_common::ids::{FileId, Span};
use luna_ast::Visibility;
use luna_semantic::symbol::{ScopeKind, SymbolKind, SymbolTable};

#[test]
fn test_macro_symbol_declaration_and_lookup() {
    let mut table = SymbolTable::new();
    let global_scope = luna_semantic::ScopeId(0);
    let mut diags = Vec::new();

    let span = Span::new(FileId(0), 0, 5);
    let sym_id = table.declare_symbol(
        "my_macro".to_string(),
        SymbolKind::Macro,
        global_scope,
        span,
        None,
        Visibility::Public,
        &mut diags,
    );

    assert!(diags.is_empty());
    assert_eq!(table.lookup_macro(&["my_macro"], global_scope), Some(sym_id));
    assert_eq!(table.lookup_macro(&["unknown_macro"], global_scope), None);
}

#[test]
fn test_macro_shadowing_in_inner_scope() {
    let mut table = SymbolTable::new();
    let global_scope = luna_semantic::ScopeId(0);
    let child_scope = table.create_scope(ScopeKind::Block, Some(global_scope));
    let mut diags = Vec::new();

    let span1 = Span::new(FileId(0), 0, 5);
    let outer_sym = table.declare_symbol(
        "foo".to_string(),
        SymbolKind::Macro,
        global_scope,
        span1,
        None,
        Visibility::Public,
        &mut diags,
    );

    let span2 = Span::new(FileId(0), 10, 15);
    let inner_sym = table.declare_symbol(
        "foo".to_string(),
        SymbolKind::Macro,
        child_scope,
        span2,
        None,
        Visibility::Private,
        &mut diags,
    );

    assert!(diags.is_empty());
    assert_ne!(outer_sym, inner_sym);

    // Inner scope lookup resolves to inner macro
    assert_eq!(table.lookup_macro(&["foo"], child_scope), Some(inner_sym));

    // Outer scope lookup resolves to outer macro
    assert_eq!(table.lookup_macro(&["foo"], global_scope), Some(outer_sym));
}

#[test]
fn test_macro_qualified_module_lookup() {
    let mut table = SymbolTable::new();
    let global_scope = luna_semantic::ScopeId(0);
    let mod_scope = table.create_scope(ScopeKind::Module, Some(global_scope));
    let mut diags = Vec::new();

    let mod_span = Span::new(FileId(0), 0, 4);
    let mod_sym = table.declare_symbol(
        "math".to_string(),
        SymbolKind::Module,
        global_scope,
        mod_span,
        None,
        Visibility::Public,
        &mut diags,
    );
    table.set_inner_scope(mod_sym, mod_scope);

    let macro_span = Span::new(FileId(0), 10, 14);
    let macro_sym = table.declare_symbol(
        "calc".to_string(),
        SymbolKind::Macro,
        mod_scope,
        macro_span,
        None,
        Visibility::Public,
        &mut diags,
    );

    assert!(diags.is_empty());

    // Direct lookup from global scope fails
    assert_eq!(table.lookup_macro(&["calc"], global_scope), None);

    // Qualified lookup math::calc succeeds
    assert_eq!(table.lookup_macro(&["math", "calc"], global_scope), Some(macro_sym));

    // Nonexistent macro in math fails
    assert_eq!(table.lookup_macro(&["math", "missing"], global_scope), None);
}

#[test]
fn test_macro_duplicate_symbol_error() {
    let mut table = SymbolTable::new();
    let global_scope = luna_semantic::ScopeId(0);
    let mut diags = Vec::new();

    let span1 = Span::new(FileId(0), 0, 5);
    let span2 = Span::new(FileId(0), 10, 15);

    table.declare_symbol(
        "dup".to_string(),
        SymbolKind::Macro,
        global_scope,
        span1,
        None,
        Visibility::Public,
        &mut diags,
    );

    table.declare_symbol(
        "dup".to_string(),
        SymbolKind::Macro,
        global_scope,
        span2,
        None,
        Visibility::Public,
        &mut diags,
    );

    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("Duplicate definition of symbol `dup` in the same scope"));
}

#[test]
fn test_macro_not_returned_for_non_macro() {
    let mut table = SymbolTable::new();
    let global_scope = luna_semantic::ScopeId(0);
    let mut diags = Vec::new();

    let span = Span::new(FileId(0), 0, 5);
    table.declare_symbol(
        "val".to_string(),
        SymbolKind::Variable,
        global_scope,
        span,
        None,
        Visibility::Public,
        &mut diags,
    );

    assert!(diags.is_empty());
    // Regular lookup finds it
    assert!(table.lookup("val", global_scope).is_some());
    // Macro lookup ignores non-macro symbols
    assert_eq!(table.lookup_macro(&["val"], global_scope), None);
}
