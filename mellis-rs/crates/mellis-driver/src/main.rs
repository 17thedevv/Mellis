use mellis_common::CompilerSession;
use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args: Vec<String> = env::args().collect();
    args.remove(0);

    let mut quiet = false;
    let mut file_path = None;

    for arg in args {
        if arg == "--quiet" {
            quiet = true;
        } else {
            file_path = Some(arg);
        }
    }

    let file_path = match file_path {
        Some(path) => path,
        None => {
            eprintln!("Usage: mellis-rs [--quiet] <file.ms>");
            process::exit(1);
        }
    };

    let source = fs::read_to_string(&file_path).unwrap_or_else(|err| {
        eprintln!("Error reading file {}: {}", file_path, err);
        process::exit(1);
    });

    let mut session = CompilerSession::new();
    session
        .source_manager
        .add_file(file_path.clone(), source.clone());

    let options = mellis_driver::CompilerOptions {
        quiet,
        ..Default::default()
    };
    match mellis_driver::compile(&file_path, &source, &options) {
        Ok(_) => {
            if !quiet {
                println!("Compiled successfully.");
            }
            process::exit(0);
        }
        Err(diagnostics) => {
            eprintln!("Number of diagnostics: {}", diagnostics.len());
            for diag in diagnostics {
                eprintln!("RAW DIAGNOSTIC: {:?}", diag);
                eprintln!("{}", diag.render(&session.source_manager));
            }
            process::exit(1);
        }
    }
}
