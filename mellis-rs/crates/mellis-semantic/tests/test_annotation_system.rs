use mellis_ast::AstArena;
use mellis_common::ids::FileId;
use mellis_lexer::lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::{AttributeProcessor, Resolver, SemanticContext};

fn parse_and_process(src: &str, allow_internal: bool) -> (SemanticContext, Vec<mellis_common::Diagnostic>) {
    let mut arena = AstArena::new();
    let mut source_manager = mellis_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), src.to_string());
    let lexer = Lexer::new(src, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = match parser.parse_file() {
        Ok(items) => items,
        Err(_) => return (SemanticContext::new(), parser.diagnostics),
    };
    if !parser.diagnostics.is_empty() {
        return (SemanticContext::new(), parser.diagnostics);
    }

    let mut src_mut = src.to_string();
    let mut attr_processor = AttributeProcessor::new(&mut arena, &mut source_manager, file_id);
    let items = match attr_processor.process_items(items) {
        Ok(items) => items,
        Err(diags) => return (SemanticContext::new(), diags),
    };

    let mut ctx = SemanticContext::new();
    ctx.allow_internal_lang_items = allow_internal;

    let mut resolver = Resolver::new(&mut ctx, &arena, &source_manager);
    resolver.resolve_items(&items);

    let diags = ctx.diagnostics.clone();
    (ctx, diags)
}

#[test]
fn test_unknown_attribute_on_struct() {
    let src = r#"
        #[non_existent_attr]
        struct S {}
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown attribute `non_existent_attr`")), "diags: {:?}", diags);
}

#[test]
fn test_unknown_attribute_on_function() {
    let src = r#"
        #[custom_magic]
        fn foo() {}
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown attribute `custom_magic`")), "diags: {:?}", diags);
}

#[test]
fn test_unknown_attribute_on_enum_variant() {
    let src = r#"
        enum E {
            #[unknown_variant_attr]
            V1,
        }
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown attribute `unknown_variant_attr`")), "diags: {:?}", diags);
}

#[test]
fn test_unknown_attribute_on_param() {
    let src = r#"
        fn foo(#[bogus_param_attr] x: i32) {}
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown attribute `bogus_param_attr`")), "diags: {:?}", diags);
}

#[test]
fn test_unknown_attribute_on_variable() {
    let src = r#"
        #[unknown_var_attr]
        dec x = 10;
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown attribute `unknown_var_attr`")), "diags: {:?}", diags);
}

#[test]
fn test_lang_missing_argument() {
    let src = r#"
        #[lang]
        export trait MyTrait {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("requires exactly one argument")), "diags: {:?}", diags);
}

#[test]
fn test_lang_multiple_arguments() {
    let src = r#"
        #[lang("try", "drop")]
        export trait MyTrait {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("requires exactly one argument")), "diags: {:?}", diags);
}

#[test]
fn test_lang_non_string_argument() {
    let src = r#"
        #[lang(12345)]
        export trait MyTrait {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("argument must be a string literal")), "diags: {:?}", diags);
}

#[test]
fn test_lang_unknown_item() {
    let src = r#"
        #[lang("something_totally_made_up")]
        export trait MyTrait {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("unknown language item `something_totally_made_up`")), "diags: {:?}", diags);
}

#[test]
fn test_lang_unauthorized() {
    let src = r#"
        #[lang("try")]
        export trait Try {}
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("trusted compiler/core contexts")), "diags: {:?}", diags);
}

#[test]
fn test_lang_wrong_target_struct() {
    let src = r#"
        #[lang("try")]
        export struct Foo {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("requires target Trait, but found Struct")), "diags: {:?}", diags);
}

#[test]
fn test_lang_wrong_target_var() {
    let src = r#"
        #[lang("try")]
        dec x = 10;
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("`#[lang]` cannot be applied to variable declarations")), "diags: {:?}", diags);
}

#[test]
fn test_lang_wrong_target_module() {
    let src = r#"
        #[lang("try")]
        module M {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("`#[lang]` cannot be applied to module declarations")), "diags: {:?}", diags);
}

#[test]
fn test_lang_wrong_target_type_alias() {
    let src = r#"
        #[lang("try")]
        type MyAlias = i32;
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("`#[lang]` cannot be applied to type alias declarations")), "diags: {:?}", diags);
}

#[test]
fn test_lang_wrong_target_impl() {
    let src = r#"
        struct Foo {}
        #[lang("try")]
        impl Foo {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("`#[lang]` cannot be applied to impl declarations")), "diags: {:?}", diags);
}

#[test]
fn test_lang_duplicate_definition() {
    let src = r#"
        #[lang("try")]
        export trait T1 {}

        #[lang("try")]
        export trait T2 {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("is defined multiple times")), "diags: {:?}", diags);
}

#[test]
fn test_lang_duplicate_symbol() {
    let src = r#"
        #[lang("try")]
        #[lang("drop")]
        export trait MultiLang {}
    "#;
    let (_, diags) = parse_and_process(src, true);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("already registered as language item")), "diags: {:?}", diags);
}

#[test]
fn test_valid_frozen_lang_items() {
    let src = r#"
        #[lang("drop")]
        export trait Drop {
            fn drop(self: &rw Self);
        }

        #[lang("from_residual")]
        export trait FromResidual<R> {
            fn from_residual(residual: R) -> Self;
        }

        #[lang("try")]
        export trait Try {
            type Output;
            type Residual;
            fn from_output(v: Self::Output) -> Self;
            fn branch(self: Self) -> ControlFlow<Self::Residual, Self::Output>;
        }

        #[lang("control_flow")]
        export enum ControlFlow<B, C> {
            #[lang("continue")]
            Continue(C),
            #[lang("break")]
            Break(B),
        }
    "#;
    let (ctx, diags) = parse_and_process(src, true);
    assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);

    // Verify all frozen lang items are registered
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::Drop).is_some());
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::FromResidual).is_some());
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::Try).is_some());
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::ControlFlow).is_some());
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::ControlFlowContinue).is_some());
    assert!(ctx.lang_items.get(mellis_semantic::lang_item::LangItem::ControlFlowBreak).is_some());
}

#[test]
fn test_sync_noescape_valid_on_param() {
    let src = r#"
        extern fn register_cb(#[sync_noescape] ptr: *rw i32);
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
}

#[test]
fn test_sync_noescape_invalid_on_struct() {
    let src = r#"
        #[sync_noescape]
        struct BadStruct {}
    "#;
    let (_, diags) = parse_and_process(src, false);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("can only be applied to parameters")), "diags: {:?}", diags);
}
