use luna_ast::{AstArena, Item};
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::symbol::ProviderId;
use crate::registry::ModuleRegistry;
use crate::discovery::{ExternalComponentDescriptor, ComponentFormat};
use crate::session::DriverSession;
use crate::error::ExternalComponentError;

pub struct ExternalComponentLoader;

impl ExternalComponentLoader {
    fn load_binary_component(
        descriptor: &ExternalComponentDescriptor,
        global_arena: &mut AstArena,
        driver_session: &mut DriverSession,
    ) -> Result<ProviderId, ExternalComponentError> {
        let mut file = std::fs::File::open(&descriptor.entry_file).map_err(|e| {
            ExternalComponentError::ReadFailed {
                path: descriptor.entry_file.clone(),
                error: e.to_string(),
            }
        })?;

        // If AstInterface section is present in the binary library (.llib / .mlib),
        // deserialize and relocate the AST arena and items into global_arena,
        // making generic function bodies available for monomorphization (Rule 7, IMPORT-8).
        if let Ok(Some((mut provider_arena, mut provider_items, source))) =
            luna_llib::reader::MlibReader::read_ast_interface(&mut file)
        {
            let file_id = driver_session
                .compiler_session
                .source_manager
                .add_file(descriptor.entry_file.to_string_lossy().to_string(), source);

            let file_relocator = luna_ast::relocator::AstRelocator::new(
                0, 0, 0, 0, 0,
                file_id,
            );
            file_relocator.relocate_arena(&mut provider_arena);

            if driver_session.registry.is_loading(&descriptor.name) {
                return Err(ExternalComponentError::ImportFailed(vec![
                    luna_common::Diagnostic::error(format!(
                        "Cyclic module dependency detected involving external component `{}`",
                        descriptor.name
                    )),
                ]));
            }
            driver_session.registry.start_loading(&descriptor.name);

            // Recursively resolve imports for the component
            if let Err(inner_diags) = crate::importer::resolve_imports(
                &mut provider_items,
                &mut provider_arena,
                driver_session,
            ) {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::ImportFailed(inner_diags));
            }

            // Relocate AST to global arena
            let relocator = luna_ast::relocator::AstRelocator::new(
                global_arena.exprs.len() as u32,
                global_arena.stmts.len() as u32,
                global_arena.decls.len() as u32,
                global_arena.types.len() as u32,
                global_arena.pats.len() as u32,
                file_id,
            );

            relocator.relocate_arena(&mut provider_arena);

            let mut shifted_provider_items = provider_items;
            for item in &mut shifted_provider_items {
                if let Item::Decl(decl_id) = item {
                    *decl_id = relocator.shift_decl_id(*decl_id);
                }
            }

            global_arena.exprs.append(&mut provider_arena.exprs);
            global_arena.stmts.append(&mut provider_arena.stmts);
            global_arena.decls.append(&mut provider_arena.decls);
            global_arena.types.append(&mut provider_arena.types);
            global_arena.pats.append(&mut provider_arena.pats);

            // Perform semantic analysis
            let provider_id = driver_session.registry.allocate_id();
            let mut semantic_ctx = luna_semantic::SemanticContext::new();
            semantic_ctx.current_provider = Some(provider_id);
            semantic_ctx.allow_internal_lang_items = true; // External libs can use internal lang items
            driver_session.registry.inject_into_ctx(&mut semantic_ctx);

            let mut resolver = luna_semantic::Resolver::new(
                &mut semantic_ctx,
                global_arena,
                &driver_session.compiler_session.source_manager,
            );
            resolver.resolve_items(&shifted_provider_items);

            if !semantic_ctx.diagnostics.is_empty() {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::SemanticFailed(semantic_ctx.diagnostics));
            }

            let comptime_engine = luna_mvir::MvirComptimeEngine {
                max_steps: 1_000_000,
                max_depth: 512,
            };
            let mut typechecker = luna_semantic::TypeChecker::new_with_engine(
                &mut semantic_ctx,
                global_arena,
                &mut driver_session.compiler_session.source_manager,
                &comptime_engine,
            );
            typechecker.typecheck_items(&shifted_provider_items);

            if !semantic_ctx.diagnostics.is_empty() {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::SemanticFailed(semantic_ctx.diagnostics));
            }

            let interface = ModuleRegistry::extract_interface_from_ctx(
                descriptor.name.clone(),
                provider_id,
                &semantic_ctx,
            );
            driver_session.registry.register_external(descriptor.name.clone(), interface);
            driver_session.registry.finish_loading();

            return Ok(provider_id);
        }

        // Fallback for libraries without AstInterface: decode SemanticMetadata
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));

        let (_, _, _, semantic_metadata) = luna_llib::reader::MlibReader::read_module(&mut file).map_err(|e| {
            ExternalComponentError::InvalidLibraryInterface {
                name: descriptor.name.clone(),
                path: descriptor.entry_file.clone(),
                reason: format!("Failed to read LLIB: {:?}", e),
            }
        })?;

        let semantic = semantic_metadata.ok_or_else(|| {
            ExternalComponentError::InvalidLibraryInterface {
                name: descriptor.name.clone(),
                path: descriptor.entry_file.clone(),
                reason: "SemanticMetadata section is missing".to_string(),
            }
        })?;

        let provider_id = driver_session.registry.allocate_id();
        let decoder = crate::metadata_decoder::InterfaceDecoder::new(
            provider_id,
            descriptor.name.clone(),
            semantic,
            driver_session.registry.providers.clone(),
        );
        let mut provider_interface = decoder.decode();
        provider_interface.name = descriptor.name.clone();

        driver_session.registry.interfaces.insert(provider_id, provider_interface);
        driver_session.registry.providers.insert(descriptor.name.clone(), provider_id);

        Ok(provider_id)
    }
    pub fn load_component(
        descriptor: &ExternalComponentDescriptor,
        global_arena: &mut AstArena,
        
        driver_session: &mut DriverSession,
    ) -> Result<ProviderId, ExternalComponentError> {
        // Ensure load-once: return existing provider if already loaded
        if let Some(&existing_id) = driver_session.registry.providers.get(&descriptor.name) {
            return Ok(existing_id);
        }

        if descriptor.format == ComponentFormat::Llib || descriptor.format == ComponentFormat::Mlib {
            return Self::load_binary_component(descriptor, global_arena, driver_session);
        }

        if descriptor.format == ComponentFormat::Mlib {
            return Err(ExternalComponentError::UnsupportedFormat {
                format: "mlib".to_string(),
                path: descriptor.entry_file.clone(),
            });
        }

        if driver_session.registry.is_loading(&descriptor.name) {
            return Err(ExternalComponentError::ImportFailed(vec![
                luna_common::Diagnostic::error(format!(
                    "Cyclic module dependency detected involving external component `{}`",
                    descriptor.name
                )),
            ]));
        }

        driver_session.registry.start_loading(&descriptor.name);

        let input = match std::fs::read_to_string(&descriptor.entry_file) {
            Ok(s) => s,
            Err(e) => {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::ReadFailed {
                    path: descriptor.entry_file.clone(),
                    error: e.to_string(),
                });
            }
        };

        let file_id = driver_session
            .compiler_session
            .source_manager
            .add_file(descriptor.entry_file.to_string_lossy().to_string(), input.clone());

        let lexer = Lexer::new(&input, file_id);
        let mut provider_arena = AstArena::new();
        let mut parser = Parser::new(lexer, &mut provider_arena, file_id);

        let parse_res = parser.parse_file();
        if !parser.diagnostics.is_empty() {
            driver_session.registry.finish_loading();
            return Err(ExternalComponentError::ParseFailed(parser.diagnostics));
        }

        let mut provider_items = match parse_res {
            Ok(items) => items,
            Err(_) => {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::ParseFailed(vec![
                    luna_common::Diagnostic::error(format!(
                        "Failed to parse external component `{}`",
                        descriptor.entry_file.display()
                    )),
                ]));
            }
        };

        // Recursively resolve imports for the component using the unified driver session
        if let Err(inner_diags) = crate::importer::resolve_imports(
            &mut provider_items,
            &mut provider_arena,
            driver_session,
        ) {
            driver_session.registry.finish_loading();
            return Err(ExternalComponentError::ImportFailed(inner_diags));
        }

        // Process annotations for the component
        let mut attr_processor =
            luna_semantic::AttributeProcessor::new(&mut provider_arena, &mut driver_session.compiler_session.source_manager, file_id);
        let shifted_provider_items_before_macro = match attr_processor.process_items(provider_items) {
            Ok(items) => items,
            Err(e) => {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::SemanticFailed(e));
            }
        };

        // Relocate AST to global arena
        

        let relocator = luna_ast::relocator::AstRelocator::new(
            global_arena.exprs.len() as u32,
            global_arena.stmts.len() as u32,
            global_arena.decls.len() as u32,
            global_arena.types.len() as u32,
            global_arena.pats.len() as u32,
            file_id,
            
        );

        relocator.relocate_arena(&mut provider_arena);

        let mut shifted_provider_items = shifted_provider_items_before_macro;
        for item in &mut shifted_provider_items {
            if let Item::Decl(decl_id) = item {
                *decl_id = relocator.shift_decl_id(*decl_id);
            }
        }

        global_arena.exprs.append(&mut provider_arena.exprs);
        global_arena.stmts.append(&mut provider_arena.stmts);
        global_arena.decls.append(&mut provider_arena.decls);
        global_arena.types.append(&mut provider_arena.types);
        global_arena.pats.append(&mut provider_arena.pats);

        // Perform semantic analysis
        let provider_id = driver_session.registry.allocate_id();
        let mut semantic_ctx = luna_semantic::SemanticContext::new();
        semantic_ctx.current_provider = Some(provider_id);
        semantic_ctx.allow_internal_lang_items = true; // External libs can use internal lang items
        driver_session.registry.inject_into_ctx(&mut semantic_ctx);

        let mut resolver =
            luna_semantic::Resolver::new(&mut semantic_ctx, global_arena, &driver_session.compiler_session.source_manager);
        resolver.resolve_items(&shifted_provider_items);

        if !semantic_ctx.diagnostics.is_empty() {
            driver_session.registry.finish_loading();
            return Err(ExternalComponentError::SemanticFailed(semantic_ctx.diagnostics));
        }

        let comptime_engine = luna_mvir::MvirComptimeEngine {
            max_steps: 1_000_000,
            max_depth: 512,
        };
        let mut typechecker =
            luna_semantic::TypeChecker::new_with_engine(&mut semantic_ctx, global_arena, &mut driver_session.compiler_session.source_manager, &comptime_engine);
        typechecker.typecheck_items(&shifted_provider_items);

        if !semantic_ctx.diagnostics.is_empty() {
            driver_session.registry.finish_loading();
            return Err(ExternalComponentError::SemanticFailed(semantic_ctx.diagnostics));
        }

        let interface = ModuleRegistry::extract_interface_from_ctx(
            descriptor.name.clone(),
            provider_id,
            &semantic_ctx,
        );
        driver_session.registry.register_external(descriptor.name.clone(), interface);
        driver_session.registry.finish_loading();

        Ok(provider_id)
    }
}
