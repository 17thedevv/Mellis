use mellis_ast::AstArena;
use mellis_common::{CompilerSession, Diagnostic};
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::{SemanticContext, Resolver, TypeChecker};
use mellis_mvir::{MvirGenerator, print_module};
use mellis_backend::{LLVMBackend, TargetConfig, link_obj_to_exe};

#[derive(Default, Clone, Debug)]
pub struct CompilerOptions {
    pub output_path: Option<String>,
    pub emit_llvm: bool,
    pub emit_mvir: bool,
    pub emit_mlib: bool,
    pub quiet: bool,
}

pub fn check(file_name: &str, input: &str, quiet: bool) -> Result<(), Vec<Diagnostic>> {
    let mut session = CompilerSession::new();
    let file_id = session.source_manager.add_file(file_name.to_string(), input.to_string());
    let lexer = Lexer::new(input, file_id);
    let mut arena = AstArena::new();
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let items = parser.parse_file().map_err(|_| parser.diagnostics.clone())?;
    if !parser.diagnostics.is_empty() { return Err(parser.diagnostics); }
    let mut semantic_ctx = SemanticContext::new();
    Resolver::new(&mut semantic_ctx, &arena, input).resolve_items(&items);
    TypeChecker::new(&mut semantic_ctx, &arena, input).typecheck_items(&items);
    let mut diagnostics = semantic_ctx.diagnostics.clone();
    for function in mellis_mvir::MvirGenerator::new(&arena, &semantic_ctx, input).generate(&items).functions {
        diagnostics.extend(mellis_borrowck::borrow_check_function(&function, &semantic_ctx));
    }
    if diagnostics.is_empty() { if !quiet { println!("check passed"); } Ok(()) } else { Err(diagnostics) }
}

pub fn compile(file_name: &str, input: &str, options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let mut session = CompilerSession::new();
    let file_id = session
        .source_manager
        .add_file(file_name.to_string(), input.to_string());

    // Lexing phase
    let lexer = Lexer::new(input, file_id);

    // Parsing phase
    let mut arena = AstArena::new();
    let mut parser = Parser::new(lexer, &mut arena, file_id);

    let file_result = parser.parse_file();
    let mut all_diagnostics = session.diagnostics;
    all_diagnostics.extend(parser.diagnostics);

    if !all_diagnostics.is_empty() {
        return Err(all_diagnostics);
    }

    match file_result {
        Ok(items) => {
            if !options.quiet {
                println!("Parsed {} items", items.len());
                println!("AstArena Exprs count: {}", arena.exprs.len());
                println!("AstArena Stmts count: {}", arena.stmts.len());
                println!("AstArena Decls count: {}", arena.decls.len());
            }
            
            // Semantic phase
            let mut semantic_ctx = SemanticContext::new();
            
            let mut resolver = Resolver::new(&mut semantic_ctx, &arena, input);
            resolver.resolve_items(&items);
            
            let mut typechecker = TypeChecker::new(&mut semantic_ctx, &arena, input);
            typechecker.typecheck_items(&items);
            
            let mut monomorphizer = mellis_semantic::Monomorphizer::new(&semantic_ctx, &arena);
            monomorphizer.run(&items);
            semantic_ctx.mono_instances = monomorphizer.instances.into_iter().collect();
            
            all_diagnostics.extend(semantic_ctx.diagnostics.clone());
            if !all_diagnostics.is_empty() {
                return Err(all_diagnostics);
            }
            
            if !options.quiet {
                println!("Resolved symbols: {}", semantic_ctx.tables.expr_symbols.len());
                println!("Monomorphized instances: {}", semantic_ctx.mono_instances.len());
                
                // Print expression types
                println!("--- Expression Types ---");
                for (expr_id, ty_id) in &semantic_ctx.tables.expr_types {
                    let ty = semantic_ctx.types.get(*ty_id);
                    println!("Expr {:?}: {:?}", expr_id, ty);
                }
                println!("----------------------\n");
            }
            
            // MVIR phase
            let generator = MvirGenerator::new(&arena, &semantic_ctx, input);
            let mut module = generator.generate(&items);
            
            if !options.quiet {
                println!("\n--- Generated MVIR ---");
                println!("{}", print_module(&module));
                println!("----------------------\n");
            }
            if options.emit_mvir {
                let base_name = std::path::Path::new(file_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                let mvir_file = format!("{}.mvir", base_name);
                let _ = std::fs::write(&mvir_file, format!("{:#?}", module)); // Actually you'd probably want a proper stringifier, but this is a placeholder
            }
            
            // Borrow Checking Phase
            if !options.quiet {
                println!("\n--- Borrow Checker ---");
            }
            let mut borrowck_errors = 0;
            for func in &module.functions {
                let diagnostics = mellis_borrowck::borrow_check_function(func, &semantic_ctx);
                for diag in &diagnostics {
                    borrowck_errors += 1;
                    if !options.quiet {
                        println!("BorrowCk Error: {}", diag.message);
                    }
                }
            }
            
            if !options.quiet {
                if borrowck_errors == 0 {
                    println!("Borrow check passed!");
                }
                println!("----------------------\n");
            }

            // MVIR Verification (Pre-opt)
            if let Err(errs) = mellis_optimizer::verify_module(&module) {
                if !options.quiet {
                    println!("--- Pre-Opt MVIR Verifier Error ---");
                    for e in errs {
                        println!("{}", e);
                    }
                    println!("-----------------------------------");
                }
                return Ok(());
            }

            // Optimization phase
            if !options.quiet {
                println!("\n--- Optimizer ---");
            }
            let mut pass_manager = mellis_optimizer::PassManager::new();
            pass_manager.add_pass(Box::new(mellis_optimizer::ConstantFolding::new()));
            pass_manager.add_pass(Box::new(mellis_optimizer::DeadCodeElimination::new()));
            pass_manager.run(&mut module);
            if !options.quiet {
                println!("-----------------");
            }

            // MVIR Verification (Post-opt)
            if let Err(errs) = mellis_optimizer::verify_module(&module) {
                if !options.quiet {
                    println!("--- Post-Opt MVIR Verifier Error ---");
                    for e in errs {
                        println!("{}", e);
                    }
                    println!("------------------------------------");
                }
                return Ok(());
            }

            // MLib generation phase
            if !options.quiet {
                println!("\n--- Serializing MLib v2 ---");
            }
            let base_name = std::path::Path::new(file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let mlib_file = format!("{}.mlib", base_name);
            let mut mlib_buffer = std::fs::File::create(&mlib_file).expect("Failed to create .mlib file");
            match mellis_mlib::MlibWriter::write_module(&module, &mut mlib_buffer) {
                Ok(_) => {
                    if !options.quiet { println!("Successfully wrote {}", mlib_file); }
                }
                Err(e) => {
                    if !options.quiet { println!("Failed to write MLib: {}", e); }
                }
            }
            if !options.quiet {
                println!("---------------------------\n");
            }

            // LLVM IR / Backend phase
            if !options.quiet {
                println!("\n--- LLVM Backend (Inkwell) ---");
            }
            let llvm_context = inkwell::context::Context::create();
            let mut backend = LLVMBackend::new(&llvm_context, &module, &semantic_ctx, file_name);
            
            if let Err(e) = backend.compile() {
                if !options.quiet { println!("Backend Error: {}", e); }
                return Err(vec![Diagnostic::error(format!("Backend Error: {}", e))]);
            }
            
            // Save to file and compile — use basename so outputs go into cwd
            let ll_file = format!("{}.ll", base_name);
            let obj_file = format!("{}.obj", base_name);
            let exe_file = options.output_path.clone().unwrap_or_else(|| format!("{}.exe", base_name));
            
            let path_ll = std::path::Path::new(&ll_file);
            let path_obj = std::path::Path::new(&obj_file);
            
            if let Err(e) = backend.emit_ll(path_ll) {
                if !options.quiet { println!("Failed to emit .ll: {}", e); }
            } else {
                if !options.quiet { println!("Successfully wrote {}", ll_file); }
            }
            if !options.emit_llvm {
                let _ = std::fs::remove_file(path_ll);
            }
            
            let config = TargetConfig::default();
            if !options.quiet { println!("Target Triple: '{}'", config.triple); }
            
            if let Err(e) = backend.emit_object(path_obj, &config) {
                if !options.quiet { println!("Failed to emit .obj: {}", e); }
                return Err(vec![Diagnostic::error(format!("Emit Object Error: {}", e))]);
            } else {
                if !options.quiet { println!("Successfully wrote {}", obj_file); }
            }
            
            if !options.quiet { println!("Linking to {}...", exe_file); }
            match link_obj_to_exe(&obj_file, &exe_file) {
                Ok(_) => {
                    if !options.quiet { println!("Build successful: {}", exe_file); }
                }
                Err(e) => {
                    if !options.quiet { println!("Link failed: {}", e); }
                    return Err(vec![Diagnostic::error(format!("Link Error: {}", e))]);
                }
            }
            
        }
        Err(e) => {
            if !options.quiet { println!("Failed to parse file."); }
        }
    }

    if all_diagnostics.is_empty() {
        Ok(())
    } else {
        Err(all_diagnostics)
    }
}

pub fn render_diagnostics(input: &str, diagnostics: &[Diagnostic]) -> String {
    let mut session = CompilerSession::new();
    session
        .source_manager
        .add_file("dummy.ms".to_string(), input.to_string());

    diagnostics
        .iter()
        .map(|d| d.render(&session.source_manager))
        .collect::<Vec<_>>()
        .join("\n")
}
