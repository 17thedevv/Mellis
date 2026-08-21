use clap::Parser;
use std::{fs, path::PathBuf, process};

#[derive(Parser, Debug)]
#[command(name = "mellis", about = "Official Mellis Compiler CLI", version)]
struct Cli {
    /// The input source file
    file: PathBuf,

    /// Output executable file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Emit intermediate representations: mvir, llvm, mlib
    #[arg(long, value_name = "TYPE")]
    emit: Option<String>,

    /// Add a directory to the library search path (not fully implemented yet)
    #[arg(short = 'L', long = "link-search", value_name = "PATH")]
    link_search: Vec<PathBuf>,

    /// Link with the given library (not fully implemented yet)
    #[arg(short = 'l', long = "link-lib", value_name = "NAME")]
    link_lib: Vec<String>,
    
    /// Suppress console output
    #[arg(short, long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    let source = fs::read_to_string(&cli.file).unwrap_or_else(|err| {
        eprintln!("Error reading file {}: {}", cli.file.display(), err);
        process::exit(1);
    });

    let mut emit_mvir = false;
    let mut emit_llvm = false;
    let mut emit_mlib = false;
    
    if let Some(emit) = &cli.emit {
        for e in emit.split(',') {
            match e.trim() {
                "mvir" => emit_mvir = true,
                "llvm" => emit_llvm = true,
                "mlib" => emit_mlib = true,
                other => {
                    eprintln!("Unknown emit type: {}", other);
                    process::exit(1);
                }
            }
        }
    }

    let options = mellis_driver::CompilerOptions {
        output_path: cli.output.map(|p| p.to_string_lossy().to_string()),
        emit_llvm,
        emit_mvir,
        emit_mlib,
        quiet: cli.quiet,
    };

    match mellis_driver::compile(cli.file.to_string_lossy().as_ref(), &source, &options) {
        Ok(_) => {
            if !cli.quiet {
                println!("Compiled successfully.");
            }
        }
        Err(diagnostics) => {
            eprintln!("{}", mellis_driver::render_diagnostics(&source, &diagnostics));
            process::exit(1);
        }
    }
}
