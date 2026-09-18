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
                error: format!("Failed to open file: {}", e),
            }
        })?;

        let manifest = luna_llib::reader::MlibReader::read_manifest(&mut file).map_err(|e| {
            ExternalComponentError::ReadFailed {
                path: descriptor.entry_file.clone(),
                error: format!("Failed to read manifest: {:?}", e),
            }
        })?;

        let mut expected_source_fingerprint = None;
        let source_path = descriptor.entry_file.with_extension("ln");
        if source_path.exists() {
            if let Ok(source_bytes) = std::fs::read(&source_path) {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&source_bytes);
                expected_source_fingerprint = Some(luna_llib::format::Fingerprint(hasher.finalize().into()));
            }
        }

        let mut expected_dependencies = std::collections::HashMap::new();
        for interface in driver_session.registry.interfaces.values() {
            expected_dependencies.insert(interface.name.clone(), interface.interface_fingerprint);
        }

        let validation_ctx = luna_llib::ValidationContext {
            expected_compiler_version: "0.1.0".to_string(),
            expected_target: luna_backend::TargetConfig::default().triple,
            expected_source_fingerprint,
            expected_dependencies,
        };

        if let Err(reason) = luna_llib::validate_artifact(&manifest, &validation_ctx) {
            return Err(ExternalComponentError::InvalidArtifact {
                name: descriptor.name.clone(),
                path: descriptor.entry_file.clone(),
                reason,
            });
        }

        use std::io::Seek;
        file.rewind().map_err(|e| ExternalComponentError::ReadFailed {
            path: descriptor.entry_file.clone(),
            error: format!("Failed to rewind file: {}", e),
        })?;

        // If AstInterface section is present in the binary library (.llib / .mlib),
        // deserialize and relocate the AST arena and items into global_arena,
        // making generic function bodies available for monomorphization (Rule 7, IMPORT-8).
        // A corrupt AstInterface section is an invalid artifact, NOT a signal to fall back
        // to SemanticMetadata reconstruction.
        let ast_interface = match luna_llib::reader::MlibReader::read_ast_interface(&mut file) {
            Ok(ast) => ast,
            Err(e) => {
                return Err(ExternalComponentError::InvalidArtifact {
                    name: descriptor.name.clone(),
                    path: descriptor.entry_file.clone(),
                    reason: format!("Corrupt AstInterface section: {:?}", e),
                });
            }
        };

        if let Some((mut provider_arena, provider_items, source)) = ast_interface {
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

            let imports = crate::importer::get_imports(
                &provider_items,
                &provider_arena,
                driver_session,
            );
            if let Err(inner_diags) = crate::importer::resolve_collected_imports(
                imports,
                global_arena,
                driver_session,
                crate::resolution_context::ProviderResolutionContext::SysrootDependency,
            ) {
                driver_session.registry.finish_loading();
                return Err(ExternalComponentError::ImportFailed(inner_diags));
            }

            // Re-validate dependency freshness now that all transitive dependencies are loaded.
            // This enforces graph-wide freshness: a stale dependency interface fingerprint must
            // reject the artifact rather than silently consume stale metadata.
            {
                let mut loaded_dependencies = std::collections::HashMap::new();
                for interface in driver_session.registry.interfaces.values() {
                    loaded_dependencies.insert(interface.name.clone(), interface.interface_fingerprint);
                }
                let dep_validation_ctx = luna_llib::ValidationContext {
                    expected_compiler_version: "0.1.0".to_string(),
                    expected_target: luna_backend::TargetConfig::default().triple,
                    expected_source_fingerprint: None,
                    expected_dependencies: loaded_dependencies,
                };
                if let Err(reason) = luna_llib::validate_artifact(&manifest, &dep_validation_ctx) {
                    driver_session.registry.finish_loading();
                    return Err(ExternalComponentError::InvalidArtifact {
                        name: descriptor.name.clone(),
                        path: descriptor.entry_file.clone(),
                        reason,
                    });
                }
            }

            let expr_start = global_arena.exprs.len() as u32;
            let decl_start = global_arena.decls.len() as u32;
            let pat_start = global_arena.pats.len() as u32;
            
            // Relocate AST to global arena
            let relocator = luna_ast::relocator::AstRelocator::new(
                expr_start,
                global_arena.stmts.len() as u32,
                decl_start,
                global_arena.types.len() as u32,
                pat_start,
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

            let ranges = crate::registry::ArenaRanges {
                exprs: expr_start..(global_arena.exprs.len() as u32),
                decls: decl_start..(global_arena.decls.len() as u32),
                pats: pat_start..(global_arena.pats.len() as u32),
            };
            let mut interface = ModuleRegistry::extract_interface_from_ctx(
                descriptor.name.clone(),
                provider_id,
                &semantic_ctx,
                &ranges,
            );
            // Propagate the artifact's canonical interface fingerprint so downstream
            // dependency freshness validation compares against the real interface identity
            // rather than the default zero fingerprint.
            interface.interface_fingerprint = manifest.provenance.interface_fingerprint;
            driver_session.registry.register_external(descriptor.name.clone(), interface);
            driver_session.registry.finish_loading();

            // Collect or extract provider object code
            let sidecar_obj = descriptor.entry_file.with_extension("obj");
            if sidecar_obj.exists() {
                driver_session.add_collected_object(sidecar_obj);
            } else {
                use std::io::Seek;
                let _ = file.seek(std::io::SeekFrom::Start(0));
                if let Ok(Some(obj_bytes)) = luna_llib::reader::MlibReader::read_object_code(&mut file) {
                    if !obj_bytes.is_empty() {
                        let extracted_obj = descriptor.entry_file.with_extension("obj");
                        if let Ok(_) = std::fs::write(&extracted_obj, &obj_bytes) {
                            driver_session.add_collected_object(extracted_obj);
                        }
                    }
                }
            }

            return Ok(provider_id);
        }

        // Canonical current `.llib` artifacts MUST carry the AstInterface section:
        // it is the required semantic authority for source/.llib parity
        // (ARTIFACT-PARITY-01). The SemanticMetadata-only reconstruction below is
        // semantic-lossy (no trait bounds / associated types / lang items), so it is
        // restricted to legacy `.mlib` compatibility artifacts. A canonical `.llib`
        // missing AstInterface is an invalid artifact, not a fallback case.
        let is_legacy_mlib = descriptor
            .entry_file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("mlib"))
            .unwrap_or(false);
        if !is_legacy_mlib {
            return Err(ExternalComponentError::InvalidArtifact {
                name: descriptor.name.clone(),
                path: descriptor.entry_file.clone(),
                reason: "canonical .llib is missing the required AstInterface section".to_string(),
            });
        }

        // Legacy `.mlib` compatibility: decode SemanticMetadata.
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
            manifest.provenance.interface_fingerprint,
        );
        let mut provider_interface = decoder.decode();
        provider_interface.name = descriptor.name.clone();

        driver_session.registry.interfaces.insert(provider_id, provider_interface);
        driver_session.registry.providers.insert(descriptor.name.clone(), provider_id);

        // Collect or extract provider object code
        let sidecar_obj = descriptor.entry_file.with_extension("obj");
        if sidecar_obj.exists() {
            driver_session.add_collected_object(sidecar_obj);
        } else {
            use std::io::Seek;
            let _ = file.seek(std::io::SeekFrom::Start(0));
            if let Ok(Some(obj_bytes)) = luna_llib::reader::MlibReader::read_object_code(&mut file) {
                if !obj_bytes.is_empty() {
                    let extracted_obj = descriptor.entry_file.with_extension("obj");
                    if let Ok(_) = std::fs::write(&extracted_obj, &obj_bytes) {
                        driver_session.add_collected_object(extracted_obj);
                    }
                }
            }
        }

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

        if descriptor.format == ComponentFormat::Llib {
            return Self::load_binary_component(descriptor, global_arena, driver_session);
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
        let imports = crate::importer::get_imports(
            &provider_items,
            &provider_arena,
            driver_session,
        );
        if let Err(inner_diags) = crate::importer::resolve_collected_imports(
            imports,
            global_arena,
            driver_session,
            crate::resolution_context::ProviderResolutionContext::SysrootDependency,
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

        let expr_start = global_arena.exprs.len() as u32;
        let decl_start = global_arena.decls.len() as u32;
        let pat_start = global_arena.pats.len() as u32;

        let relocator = luna_ast::relocator::AstRelocator::new(
            expr_start,
            global_arena.stmts.len() as u32,
            decl_start,
            global_arena.types.len() as u32,
            pat_start,
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

        let ranges = crate::registry::ArenaRanges {
            exprs: expr_start..(global_arena.exprs.len() as u32),
            decls: decl_start..(global_arena.decls.len() as u32),
            pats: pat_start..(global_arena.pats.len() as u32),
        };
        let interface = ModuleRegistry::extract_interface_from_ctx(
            descriptor.name.clone(),
            provider_id,
            &semantic_ctx,
            &ranges,
        );
        driver_session.registry.register_external(descriptor.name.clone(), interface);
        driver_session.registry.finish_loading();

        Ok(provider_id)
    }
}
