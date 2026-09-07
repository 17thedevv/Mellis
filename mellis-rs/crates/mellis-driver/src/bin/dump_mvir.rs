use mellis_common::CompilerSession;
use mellis_driver::CompilerOptions;

fn main() {
    let file_path = "crates/mellis-driver/tests/ui/try_borrowck_conditional_move.ms";
    let source = std::fs::read_to_string(file_path).unwrap();
    let mut session = CompilerSession::new();
    session.allow_internal_lang_items = true;
    session
        .source_manager
        .add_file(file_path.to_string(), source.clone());

    let search_paths = vec![
        "../../scratch".to_string(),
        "crates/mellis-driver/tests/ui".to_string(),
        ".".to_string(),
    ];
    let options = CompilerOptions {
        quiet: false,
        emit_mvir: true,
        search_paths,
        ..Default::default()
    };

    let res = mellis_driver::compile_with_session(&mut session, file_path, source, &options);
    if let Err(diags) = res {
        for diag in diags {
            eprintln!("{}", diag.render(&session.source_manager));
        }
    }

    // Dump all SemanticTypes manually
    // Wait, the semantic_ctx is dropped inside `compile_with_session`.
    // I can't dump it from here!
}
