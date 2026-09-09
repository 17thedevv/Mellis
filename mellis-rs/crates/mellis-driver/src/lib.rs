pub mod importer;
pub mod registry;
pub mod async_lowering;
pub mod sysroot;
pub mod external;
pub mod error;
pub mod session;
pub mod discovery;

pub use session::DriverSession;

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
    pub search_paths: Vec<String>,
    pub quiet: bool,
    pub no_link: bool,
    pub comptime_steps: Option<usize>,
    pub comptime_depth: Option<usize>,
}

fn verify_items_lifetime(
    items: &[mellis_ast::Item],
    arena: &AstArena,
    semantic_ctx: &SemanticContext,
    source_manager: &mellis_common::SourceManager,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in items {
        if let mellis_ast::Item::Decl(decl_id) = item {
            let decl = &arena.decls[decl_id.0 as usize];
            match decl {
                mellis_ast::Decl::Module { items: inner_decls, .. } => {
                    let inner_items: Vec<mellis_ast::Item> = inner_decls.iter().map(|&d| mellis_ast::Item::Decl(d)).collect();
                    verify_items_lifetime(&inner_items, arena, semantic_ctx, source_manager, diagnostics);
                }
                _ => {
                    if let Err(e) = mellis_semantic::lifetime::verify_before_codegen(decl, semantic_ctx, source_manager, arena) {
                        diagnostics.push(e.into_diagnostic());
                    }
                }
            }
        }
    }
}

/// Runs the full semantic pipeline (parse → resolve → typecheck → mono → lifetime verify)
/// and returns the collected diagnostics WITHOUT entering MVIR generation.
///
/// This function exists to mechanically verify the pipeline invariant:
///   "Semantic-invalid AST MUST NOT enter MVIR generation."
///
/// Returns `Err(diagnostics)` if any semantic errors are detected (same gate as `check()`),
/// or `Ok(())` if the program would have passed the semantic gate.
pub fn check_semantic_only(file_name: &str, input: String, options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let mut session = CompilerSession::new();
    let file_id = session.source_manager.add_file(file_name.to_string(), input.clone());
    let lexer = Lexer::new(&input, file_id);
    let mut arena = AstArena::new();
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let mut items = parser.parse_file().map_err(|_| parser.diagnostics.clone())?;
    if !parser.diagnostics.is_empty() { return Err(parser.diagnostics); }
    let mut semantic_ctx = SemanticContext::new();

    let search_paths_buf: Vec<std::path::PathBuf> = options.search_paths.iter().map(std::path::PathBuf::from).collect();
    let sysroot = search_paths_buf
        .iter()
        .find(|p| p.join("libs").join("external").exists())
        .map(|p| crate::sysroot::Sysroot::from_root(p.clone()))
        .or_else(|| crate::sysroot::Sysroot::discover(None).ok())
        .or_else(|| crate::sysroot::Sysroot::discover_for_test().ok())
        .unwrap_or_else(|| crate::sysroot::Sysroot::from_root(search_paths_buf.first().cloned().unwrap_or_else(|| std::path::PathBuf::from("."))));
    let mut driver_session = crate::session::DriverSession::new(sysroot, &mut session, options.search_paths.as_slice());

    if let Err(e) = driver_session.bootstrap_core(&mut arena) {
        return Err(e.into_diagnostics());
    }

    crate::importer::resolve_imports(&mut items, &mut arena, &mut driver_session).map_err(|e| e)?;
    let registry = std::mem::take(&mut driver_session.registry);
    drop(driver_session);

    let mut attr_processor = mellis_semantic::AttributeProcessor::new(&mut arena, &mut session.source_manager, file_id);
    let items = attr_processor.process_items(items).map_err(|e| e)?;

    registry.inject_into_ctx(&mut semantic_ctx);

    let mut resolver = Resolver::new(&mut semantic_ctx, &arena, &session.source_manager);
    resolver.register_macros(&items);
    if !semantic_ctx.diagnostics.is_empty() {
        return Err(semantic_ctx.diagnostics);
    }

    let mut macro_engine = mellis_semantic::MacroEngine::new(
        &mut arena,
        &session.source_manager,
        file_id,
        &semantic_ctx.symbol_table,
        &semantic_ctx.tables,
    );
    let items = macro_engine.expand_items(items).map_err(|e| e)?;

    Resolver::new(&mut semantic_ctx, &arena, &session.source_manager).resolve_items(&items);
    let comptime_engine = mellis_mvir::MvirComptimeEngine {
        max_steps: options.comptime_steps.unwrap_or(1_000_000),
        max_depth: options.comptime_depth.unwrap_or(512),
    };
    TypeChecker::new_with_engine(&mut semantic_ctx, &arena, &session.source_manager, &comptime_engine).typecheck_items(&items);

    let mut mono = mellis_semantic::MonoCollector::new(&mut semantic_ctx, &arena);
    mono.run(&items);
    semantic_ctx.instantiated_functions = mono.instantiated.into_values().collect();

    let mut diagnostics = semantic_ctx.diagnostics.clone();
    verify_items_lifetime(&items, &arena, &semantic_ctx, &session.source_manager, &mut diagnostics);
    let diagnostics: Vec<_> = diagnostics.into_iter().fold(Vec::new(), |mut acc, d| {
        if !acc.contains(&d) { acc.push(d); }
        acc
    });

    // This is the MVIR gate. We stop here — MVIR generation is NOT invoked.
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

pub fn check(file_name: &str, input: String, options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let mut session = CompilerSession::new();
    let file_id = session.source_manager.add_file(file_name.to_string(), input.clone());
    let lexer = Lexer::new(&input, file_id);
    let mut arena = AstArena::new();
    let mut parser = Parser::new(lexer, &mut arena, file_id);
    let mut items = parser.parse_file().map_err(|_| parser.diagnostics.clone())?;
    if !parser.diagnostics.is_empty() { return Err(parser.diagnostics); }
    let mut semantic_ctx = SemanticContext::new();
    
    let search_paths_buf: Vec<std::path::PathBuf> = options.search_paths.iter().map(std::path::PathBuf::from).collect();
    let sysroot = search_paths_buf
        .iter()
        .find(|p| p.join("libs").join("external").exists())
        .map(|p| crate::sysroot::Sysroot::from_root(p.clone()))
        .or_else(|| crate::sysroot::Sysroot::discover(None).ok())
        .or_else(|| crate::sysroot::Sysroot::discover_for_test().ok())
        .unwrap_or_else(|| crate::sysroot::Sysroot::from_root(search_paths_buf.first().cloned().unwrap_or_else(|| std::path::PathBuf::from("."))));
    let mut driver_session = crate::session::DriverSession::new(sysroot, &mut session, options.search_paths.as_slice());
    
    if let Err(e) = driver_session.bootstrap_core(&mut arena) {
        return Err(e.into_diagnostics());
    }
    
    crate::importer::resolve_imports(&mut items, &mut arena, &mut driver_session).map_err(|e| e)?;
    let registry = std::mem::take(&mut driver_session.registry);
    drop(driver_session);
    
    let mut attr_processor = mellis_semantic::AttributeProcessor::new(&mut arena, &mut session.source_manager, file_id);
    let items = attr_processor.process_items(items).map_err(|e| e)?;

    registry.inject_into_ctx(&mut semantic_ctx);

    let mut resolver = Resolver::new(&mut semantic_ctx, &arena, &session.source_manager);
    resolver.register_macros(&items);
    if !semantic_ctx.diagnostics.is_empty() {
        return Err(semantic_ctx.diagnostics);
    }

    let mut macro_engine = mellis_semantic::MacroEngine::new(
        &mut arena,
        &session.source_manager,
        file_id,
        &semantic_ctx.symbol_table,
        &semantic_ctx.tables,
    );
    let items = macro_engine.expand_items(items).map_err(|e| e)?;

    Resolver::new(&mut semantic_ctx, &arena, &session.source_manager).resolve_items(&items);
    let comptime_engine = mellis_mvir::MvirComptimeEngine {
        max_steps: options.comptime_steps.unwrap_or(1_000_000),
        max_depth: options.comptime_depth.unwrap_or(512),
    };
    TypeChecker::new_with_engine(&mut semantic_ctx, &arena, &session.source_manager, &comptime_engine).typecheck_items(&items);
    
    let mut mono = mellis_semantic::MonoCollector::new(&mut semantic_ctx, &arena);
    mono.run(&items);
    semantic_ctx.instantiated_functions = mono.instantiated.into_values().collect();
    
    let mut diagnostics = semantic_ctx.diagnostics.clone();
    verify_items_lifetime(&items, &arena, &semantic_ctx, &session.source_manager, &mut diagnostics);
    let mut diagnostics: Vec<_> = diagnostics.into_iter().fold(Vec::new(), |mut acc, d| {
        if !acc.contains(&d) { acc.push(d); }
        acc
    });
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    
    let (module, mvir_diags) = mellis_mvir::MvirGenerator::new(&arena, &semantic_ctx, &session.source_manager).generate(&items);
    if !mvir_diags.is_empty() {
        return Err(mvir_diags);
    }
    
    let mut interproc = mellis_borrowck::interprocedural::InterproceduralContext::new();
    interproc.compute_summaries(&module);
    
    for function in module.functions {
        let (diags, _) = mellis_borrowck::borrow_check_function(&function, &semantic_ctx, &interproc.summaries);
        diagnostics.extend(diags);
    }
    if diagnostics.is_empty() { if !options.quiet { println!("check passed"); } Ok(()) } else { Err(diagnostics) }
}

pub fn compile(file_name: &str, input: String, options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let mut session = CompilerSession::new();
    compile_with_session(&mut session, file_name, input, options)
}

pub fn check_and_render(file_name: &str, input: String, options: &CompilerOptions) -> Result<(), String> {
    let session = CompilerSession::new();
    match check(file_name, input, options) {
        Ok(()) => Ok(()),
        Err(diags) => Err(diags.iter().map(|d| d.render(&session.source_manager)).collect::<Vec<_>>().join("\n")),
    }
}

pub fn compile_and_render(file_name: &str, input: String, options: &CompilerOptions) -> Result<(), String> {
    let mut session = CompilerSession::new();
    match compile_with_session(&mut session, file_name, input, options) {
        Ok(()) => Ok(()),
        Err(diags) => Err(diags.iter().map(|d| d.render(&session.source_manager)).collect::<Vec<_>>().join("\n")),
    }
}

pub fn compile_with_session(session: &mut CompilerSession, file_name: &str, input: String, options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let file_id = session
        .source_manager
        .add_file(file_name.to_string(), input.clone());

    // Lexing phase
    let lexer = Lexer::new(&input, file_id);

    // Parsing phase
    let mut arena = AstArena::new();
    let mut parser = Parser::new(lexer, &mut arena, file_id);

    let file_result = parser.parse_file();
    let mut all_diagnostics = session.diagnostics.clone();
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
            
            
            let mut items_mut = items.clone();
            
            let search_paths_buf: Vec<std::path::PathBuf> = options.search_paths.iter().map(std::path::PathBuf::from).collect();
            let sysroot = search_paths_buf
                .iter()
                .find(|p| p.join("libs").join("external").exists())
                .map(|p| crate::sysroot::Sysroot::from_root(p.clone()))
                .or_else(|| crate::sysroot::Sysroot::discover(None).ok())
                .or_else(|| crate::sysroot::Sysroot::discover_for_test().ok())
                .unwrap_or_else(|| crate::sysroot::Sysroot::from_root(search_paths_buf.first().cloned().unwrap_or_else(|| std::path::PathBuf::from("."))));
            let mut driver_session = crate::session::DriverSession::new(sysroot, session, options.search_paths.as_slice());
            
            if let Err(e) = driver_session.bootstrap_core(&mut arena) {
                return Err(e.into_diagnostics());
            }
            
            if let Err(e) = crate::importer::resolve_imports(&mut items_mut, &mut arena, &mut driver_session) {
                return Err(e);
            }
            
            let registry = std::mem::take(&mut driver_session.registry);
            drop(driver_session);

            let mut attr_processor = mellis_semantic::AttributeProcessor::new(&mut arena, &mut session.source_manager, file_id);
            let items_mut = attr_processor.process_items(items_mut).map_err(|e| e)?;

            registry.inject_into_ctx(&mut semantic_ctx);

            let mut resolver = Resolver::new(&mut semantic_ctx, &arena, &session.source_manager);
            resolver.register_macros(&items_mut);
            if !semantic_ctx.diagnostics.is_empty() {
                return Err(semantic_ctx.diagnostics);
            }

            let mut macro_engine = mellis_semantic::MacroEngine::new(
                &mut arena,
                &session.source_manager,
                file_id,
                &semantic_ctx.symbol_table,
                &semantic_ctx.tables,
            );
            let items_mut = macro_engine.expand_items(items_mut).map_err(|e| e)?;

            let mut resolver = Resolver::new(&mut semantic_ctx, &arena, &session.source_manager);
            resolver.resolve_items(&items_mut);
            
            let comptime_engine = mellis_mvir::MvirComptimeEngine {
                max_steps: options.comptime_steps.unwrap_or(1_000_000),
                max_depth: options.comptime_depth.unwrap_or(512),
            };
            let mut typechecker = TypeChecker::new_with_engine(&mut semantic_ctx, &arena, &session.source_manager, &comptime_engine);
            typechecker.typecheck_items(&items_mut);
            
            let mut mono = mellis_semantic::MonoCollector::new(&mut semantic_ctx, &arena);
            mono.run(&items_mut);
            semantic_ctx.instantiated_functions = mono.instantiated.into_values().collect();
            
            for diag in semantic_ctx.diagnostics.clone() {
                if !all_diagnostics.contains(&diag) {
                    all_diagnostics.push(diag);
                }
            }
            if !all_diagnostics.is_empty() {
                return Err(all_diagnostics);
            }
            
            if !options.quiet {
                println!("Resolved expr symbols: {}", semantic_ctx.tables.expr_symbols.len());
                println!("Resolved total symbols: {}", semantic_ctx.symbol_table.symbols.len());
                println!("Monomorphized instances: {}", semantic_ctx.instantiated_functions.len());
                
                // Print expression types
                println!("--- Expression Types ---");
                for (expr_id, ty_id) in &semantic_ctx.tables.expr_types {
                    let ty = semantic_ctx.types.get(*ty_id);
                    println!("Expr {:?}: {:?}", expr_id, ty);
                }
                println!("----------------------\n");
            }
            
            // MVIR phase
            verify_items_lifetime(&items_mut, &arena, &semantic_ctx, &session.source_manager, &mut all_diagnostics);
            if !all_diagnostics.is_empty() {
                return Err(all_diagnostics);
            }
            let generator = MvirGenerator::new(&arena, &semantic_ctx, &session.source_manager);
            let (mut module, mvir_diags) = generator.generate(&items_mut);
            if !mvir_diags.is_empty() {
                return Err(mvir_diags);
            }
            
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
            
            let mut interproc = mellis_borrowck::interprocedural::InterproceduralContext::new();
            interproc.compute_summaries(&module);
            let summaries = interproc.summaries;
            
            let mut borrowck_errors = 0;
            for func in &mut module.functions {
                let (diagnostics, redundant_drops) = mellis_borrowck::borrow_check_function(func, &semantic_ctx, &summaries);
                borrowck_errors += diagnostics.len();
                if !options.quiet {
                    for diag in &diagnostics {
                        println!("BorrowCk Error: {}", diag.message);
                    }
                }
                mellis_borrowck::cleanup::eliminate_redundant_drops(func, &redundant_drops);
                all_diagnostics.extend(diagnostics);
            }

            if borrowck_errors != 0 {
                return Err(all_diagnostics);
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
            let mlib_file = if options.emit_mlib && options.output_path.is_some() {
                options.output_path.as_ref().unwrap().clone()
            } else {
                format!("{}.mlib", base_name)
            };
            let mut mlib_buffer = std::fs::File::create(&mlib_file).expect("Failed to create .mlib file");
            let manifest = mellis_mlib::Manifest {
                identity: mellis_mlib::ArtifactIdentity {
                    package_id: "".to_string(),
                    version: "0.1.0".to_string(),
                    module_id: "".to_string(),
                    artifact_id: "".to_string(),
                },
                target: mellis_mlib::TargetContract {
                    target_triple: "".to_string(),
                    object_format: "ELF".to_string(),
                    abi: "".to_string(),
                    pointer_width: 64,
                    endianness: "".to_string(),
                },
                dependencies: mellis_mlib::DependencyTable {
                    mlib_deps: vec![],
                    native_deps: vec![],
                },
                object_metadata: None,
                provenance: mellis_mlib::Provenance {
                    source_fingerprint: [0; 32],
                    compiler_version: "0.1.0".to_string(),
                    codegen_options: "".to_string(),
                    interface_hash: [0; 32],
                },
                export_table: None,
            };
            
            match mellis_mlib::MlibWriter::write_module(&module, &arena, &items, &input, manifest, None, &mut mlib_buffer) {
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
                println!("\n--- Async Lowering & LLVM Backend ---");
            }
            
            // Async Lowering Phase (target specific, runs after MLib)
            async_lowering::lower_async(&mut module, &mut semantic_ctx);
            
            // DEBUG: Print the lowered module
            println!("--- Lowered MVIR ---");
            println!("{}", mellis_mvir::printer::print_module(&module));
            println!("--------------------");
            let llvm_context = inkwell::context::Context::create();
            let mut backend = LLVMBackend::new(&llvm_context, &module, &semantic_ctx, file_name);
            
            if let Err(e) = backend.compile() {
                if !options.quiet { println!("Backend Error: {}", e); }
                return Err(vec![Diagnostic::error(format!("Backend Error: {}", e))]);
            }
            
            // Save to file and compile — if output_path is specified, place .ll and .obj alongside it
            let (ll_file, obj_file, exe_file) = if let Some(ref out) = options.output_path {
                let out_path = std::path::Path::new(out);
                let parent = out_path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or(base_name);
                (
                    parent.join(format!("{}.ll", stem)).to_string_lossy().to_string(),
                    parent.join(format!("{}.obj", stem)).to_string_lossy().to_string(),
                    out.clone(),
                )
            } else {
                (
                    format!("{}.ll", base_name),
                    format!("{}.obj", base_name),
                    format!("{}.exe", base_name),
                )
            };
            
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
            
            if !options.no_link {
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
            
        }
        Err(_e) => {
            if !options.quiet { println!("Failed to parse file."); }
        }
    }

    if all_diagnostics.is_empty() {
        Ok(())
    } else {
        Err(all_diagnostics)
    }
}


