use mellis_ast::AstArena;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::{SemanticContext, Resolver, TypeChecker};
use mellis_common::ids::FileId;

fn check_exhaustiveness(source: &str) -> bool {
    let mut arena = AstArena::new();
    let mut source_manager = mellis_common::source::SourceManager::new();
    let file_id = source_manager.add_file("test.ms".to_string(), source.to_string());
    let lexer = Lexer::new(source, file_id);
    let mut parser = Parser::new(lexer, &mut arena, file_id);

    let items = match parser.parse_file() {
        Ok(items) => items,
        Err(_) => panic!("Failed to parse valid source"),
    };

    let mut semantic_ctx = SemanticContext::new();
    let mut resolver = Resolver::new(&mut semantic_ctx, &arena, &source_manager);
    resolver.resolve_items(&items);
    
    let mut typechecker = TypeChecker::new(&mut semantic_ctx, &arena, &source_manager);
    typechecker.typecheck_items(&items);
    
    // Return true if NO diagnostic contains "Match is not exhaustive"
    !semantic_ctx.diagnostics.iter().any(|d| d.message.contains("Match is not exhaustive"))
}

#[test]
fn test_fuzz_exhaustiveness_property() {
    let mut seed = 54321;
    let mut rand = || -> usize {
        seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF;
        seed
    };

    // We will generate 100 bool matches and 100 enum matches
    // and verify that the checker perfectly separates exhaustive from non-exhaustive
    
    for _ in 0..100 {
        let is_exhaustive_ground_truth = (rand() % 2) == 0;
        let mut source = String::new();
        source.push_str("fn test_func(a: bool) {\n    match a {\n");
        
        if is_exhaustive_ground_truth {
            // Must have both true and false, or a wildcard
            let kind = rand() % 3;
            if kind == 0 {
                source.push_str("        true -> { },\n        false -> { },\n");
            } else if kind == 1 {
                source.push_str("        true -> { },\n        _ -> { },\n");
            } else {
                source.push_str("        _ -> { },\n");
            }
        } else {
            // Missing one or both
            let kind = rand() % 2;
            if kind == 0 {
                source.push_str("        true -> { },\n");
            } else {
                source.push_str("        false -> { },\n");
            }
        }
        source.push_str("    }\n}\n");

        let checker_says_exhaustive = check_exhaustiveness(&source);
        assert_eq!(checker_says_exhaustive, is_exhaustive_ground_truth, 
            "Mismatch on source:\n{}", source);
    }
    
    for _ in 0..100 {
        let is_exhaustive_ground_truth = (rand() % 2) == 0;
        let mut source = String::new();
        source.push_str("enum MyEnum { V1, V2, V3 }\n");
        source.push_str("fn test_func(e: MyEnum) {\n    match e {\n");
        
        if is_exhaustive_ground_truth {
            // Must have V1, V2, V3, or wildcard
            let kind = rand() % 3;
            if kind == 0 {
                source.push_str("        MyEnum::V1 -> { },\n        MyEnum::V2 -> { },\n        MyEnum::V3 -> { },\n");
            } else if kind == 1 {
                source.push_str("        MyEnum::V1 -> { },\n        MyEnum::V2 -> { },\n        _ -> { },\n");
            } else {
                source.push_str("        _ -> { },\n");
            }
        } else {
            // Missing one or more
            let kind = rand() % 3;
            if kind == 0 {
                source.push_str("        MyEnum::V1 -> { },\n");
            } else if kind == 1 {
                source.push_str("        MyEnum::V1 -> { },\n        MyEnum::V2 -> { },\n");
            } else {
                source.push_str("        MyEnum::V3 -> { },\n");
            }
        }
        source.push_str("    }\n}\n");

        let checker_says_exhaustive = check_exhaustiveness(&source);
        assert_eq!(checker_says_exhaustive, is_exhaustive_ground_truth, 
            "Mismatch on source:\n{}", source);
    }
}
