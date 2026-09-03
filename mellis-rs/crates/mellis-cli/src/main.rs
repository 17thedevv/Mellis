use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf, process};

#[derive(Parser, Debug)]
#[command(name = "mellis", about = "Official Mellis Compiler CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a target (.exe or .mlib)
    Build {
        /// The input source file
        file: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Emit intermediate representations: mvir, llvm, mlib
        #[arg(long, value_name = "TYPE")]
        emit: Option<String>,

        /// Suppress console output
        #[arg(short, long)]
        quiet: bool,

        /// Build as a library (do not link into an executable)
        #[arg(long)]
        lib: bool,

        /// Add a directory to the module search path
        #[arg(short = 'I', long = "search-path", value_name = "DIR")]
        search_paths: Vec<PathBuf>,
    },
    /// Check a source file for errors without emitting output
    Check {
        /// The input source file
        file: PathBuf,

        /// Suppress console output
        #[arg(short, long)]
        quiet: bool,

        /// Add a directory to the module search path
        #[arg(short = 'I', long = "search-path", value_name = "DIR")]
        search_paths: Vec<PathBuf>,
    },
    /// Compile and run an application
    Run {
        /// The input source file
        file: PathBuf,

        /// Suppress console output
        #[arg(short, long)]
        quiet: bool,

        /// Add a directory to the module search path
        #[arg(short = 'I', long = "search-path", value_name = "DIR")]
        search_paths: Vec<PathBuf>,
    },
    /// Inspect the manifest of a given .mlib file
    Manifest {
        /// The .mlib file to inspect
        file: PathBuf,
    },
}

fn parse_emit(emit_opt: &Option<String>) -> (bool, bool, bool) {
    let mut emit_mvir = false;
    let mut emit_llvm = false;
    let mut emit_mlib = false;

    if let Some(emit) = emit_opt {
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
    (emit_mvir, emit_llvm, emit_mlib)
}

fn read_source(file: &PathBuf) -> String {
    fs::read_to_string(file).unwrap_or_else(|err| {
        eprintln!("Error reading file {}: {}", file.display(), err);
        process::exit(1);
    })
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Manifest { file } => {
            let mut f = std::fs::File::open(file).unwrap_or_else(|err| {
                eprintln!("Error opening {}: {}", file.display(), err);
                process::exit(1);
            });
            match mellis_mlib::MlibReader::read_manifest(&mut f) {
                Ok(manifest) => {
                    let json = serde_json::to_string_pretty(&manifest).expect("Failed to serialize manifest to JSON");
                    println!("{}", json);
                    process::exit(0);
                }
                Err(e) => {
                    eprintln!("Failed to read manifest: {:?}", e);
                    process::exit(1);
                }
            }
        }
        Commands::Build { file, output, emit, quiet, lib, search_paths } => {
            let source = read_source(file);
            let (emit_mvir, emit_llvm, emit_mlib) = parse_emit(emit);
            
            let options = mellis_driver::CompilerOptions {
                output_path: output.as_ref().map(|p| p.to_string_lossy().to_string()),
                emit_llvm,
                emit_mvir,
                emit_mlib,
                quiet: *quiet,
                search_paths: search_paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                no_link: *lib,
            };

            if let Err(rendered_diagnostics) = mellis_driver::compile_and_render(file.to_string_lossy().as_ref(), source.clone(), &options) {
                eprintln!("{}", rendered_diagnostics);
                process::exit(1);
            }
            if !*quiet {
                println!("Build successfully.");
            }
        }
        Commands::Check { file, quiet, search_paths } => {
            let source = read_source(file);
            
            let options = mellis_driver::CompilerOptions {
                output_path: None,
                emit_llvm: false,
                emit_mvir: false,
                emit_mlib: false,
                quiet: *quiet,
                search_paths: search_paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                no_link: false,
            };

            if let Err(rendered_diagnostics) = mellis_driver::compile_and_render(file.to_string_lossy().as_ref(), source.clone(), &options) {
                eprintln!("{}", rendered_diagnostics);
                process::exit(1);
            }
            if !*quiet {
                println!("Check successfully (no errors found).");
            }
        }
        Commands::Run { file, quiet, search_paths } => {
            let source = read_source(file);
            
            let options = mellis_driver::CompilerOptions {
                output_path: None, // Will compile to temp dir
                emit_llvm: false,
                emit_mvir: false,
                emit_mlib: false,
                quiet: *quiet,
                search_paths: search_paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                no_link: false,
            };

            if let Err(rendered_diagnostics) = mellis_driver::compile_and_render(file.to_string_lossy().as_ref(), source.clone(), &options) {
                eprintln!("{}", rendered_diagnostics);
                process::exit(1);
            }
        }
    }
}
