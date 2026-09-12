use std::path::{Path, PathBuf};
use luna_ast::AstArena;
use luna_common::Diagnostic;
use luna_semantic::{SemanticContext, Resolver, ScopeId};

use crate::module::{ModuleName, ModuleRegistry, ModuleOrigin, BeginLoadResult};
use crate::discovery::DiscoveryResult;

/// Loads module source/artifacts and produces semantic scopes.
///
/// This is called by the driver after ModuleDiscovery resolves a name.
/// It owns the read → lex → parse → resolve pipeline for imported modules.
pub struct ModuleLoader;

impl ModuleLoader {
    /// Load a module based on a discovery result.
    /// Handles cycle detection via the registry's LoadState.
    pub fn load(
        name: &ModuleName,
        discovery_result: DiscoveryResult,
        registry: &mut ModuleRegistry,
        host_ctx: &mut SemanticContext,
        host_source_map: &mut Vec<(String, String)>,
        arena: &mut AstArena,
        search_paths: &[PathBuf],
    ) -> Result<ScopeId, Vec<Diagnostic>> {
        // Register the module with its origin
        let origin = match &discovery_result {
            DiscoveryResult::Source(path) => ModuleOrigin::Source(path.clone()),
            DiscoveryResult::Mlib(path) => ModuleOrigin::Mlib(path.clone()),
            DiscoveryResult::NotFound => {
                return Err(vec![Diagnostic::error(
                    format!("Module '{}' not found in any search path", name),
                )]);
            }
        };

        let _module_id = registry.register(name.clone(), origin);

        // Check loading state (cycle detection + load-once)
        match registry.begin_loading(name) {
            Ok(BeginLoadResult::AlreadyLoaded(id)) => {
                // Load-once: return existing scope
                let module = registry.get_by_id(id).unwrap();
                return Ok(ScopeId(module.root_scope));
            }
            Ok(BeginLoadResult::PreviouslyFailed(_)) => {
                return Err(vec![Diagnostic::error(
                    format!("Module '{}' previously failed to load", name),
                )]);
            }
            Ok(BeginLoadResult::StartedLoading(_)) => {
                // Proceed with loading
            }
            Err(cycle_err) => {
                return Err(vec![Diagnostic::error(
                    format!("Cyclic import detected: module '{}' is already being loaded", cycle_err.module_name),
                )]);
            }
        }

        // Dispatch based on discovery result
        let result = match &discovery_result {
            DiscoveryResult::Source(path) => {
                Self::load_source(path, name, host_ctx, host_source_map, arena, search_paths, registry)
            }
            DiscoveryResult::Mlib(path) => {
                Self::load_mlib(path, name, host_ctx, host_source_map, arena, search_paths, registry)
            }
            DiscoveryResult::NotFound => unreachable!("handled above"),
        };

        match result {
            Ok(scope_id) => {
                registry.finish_loading(name, scope_id.0);
                Ok(scope_id)
            }
            Err(diags) => {
                registry.mark_failed(name);
                Err(diags)
            }
        }
    }

    /// After relocating a module's AST into the host arena, scan the relocated
    /// items for `Decl::Import` and recursively load each nested dependency.
    /// Returns a `ModuleNamespaceMap` so the Resolver can resolve cross-module
    /// references (e.g. `alloc.ms` importing `core`).
    fn resolve_nested_imports(
        imported_items: &[luna_ast::Item],
        module_source: &str,
        host_ctx: &mut SemanticContext,
        host_source_map: &mut Vec<(String, String)>,
        host_arena: &mut AstArena,
        search_paths: &[PathBuf],
        registry: &mut ModuleRegistry,
    ) -> luna_semantic::ModuleNamespaceMap {
        use luna_ast::{Decl, ImportKind};

        let mut namespace_map = luna_semantic::ModuleNamespaceMap::new();

        // Collect nested imports from the relocated items
        for item in imported_items {
            if let luna_ast::Item::Decl(decl_id) = item {
                let decl = &host_arena.decls[decl_id.0 as usize];
                if let Decl::Import { name, kind, .. } = decl {
                    let name_span = *name;
                    let kind = *kind;

                    // Extract import name from source
                    let mut name_str = module_source
                        .get(name_span.start as usize..name_span.end as usize)
                        .unwrap_or("")
                        .to_string();

                    // Strip quotes from local imports
                    if name_str.starts_with('"') && name_str.ends_with('"') {
                        name_str = name_str[1..name_str.len() - 1].to_string();
                    }

                    if name_str.is_empty() {
                        continue;
                    }

                    // Skip if already loaded in this namespace
                    if namespace_map.scopes.contains_key(&name_str) {
                        continue;
                    }

                    // Build search paths for discovery
                    let mut sp = crate::discovery::SearchPaths::new();
                    for path in search_paths {
                        sp.push(path.clone());
                    }

                    let source_dir = PathBuf::from(".");
                    let discovery = crate::discovery::ModuleDiscovery::new(source_dir, sp);
                    let mod_name = ModuleName::new(&name_str);
                    let result = discovery.resolve(&mod_name, kind);

                    match Self::load(
                        &mod_name,
                        result,
                        registry,
                        host_ctx,
                        host_source_map,
                        host_arena,
                        search_paths,
                    ) {
                        Ok(scope_id) => {
                            namespace_map.scopes.insert(name_str.clone(), scope_id);
                        }
                        Err(diags) => {
                            for diag in &diags {
                                eprintln!("Nested import warning: {}", diag.message);
                            }
                        }
                    }
                }
            }
        }

        namespace_map
    }

    /// Load a `.ms` source module:
    /// read file → lex → parse → resolve → return root scope.
    ///
    /// The module's declarations are resolved into the host SemanticContext
    /// under a new Module scope, so they are accessible via `::` lookups.
    fn load_source(
        path: &Path,
        name: &ModuleName,
        host_ctx: &mut SemanticContext,
        host_source_map: &mut Vec<(String, String)>,
        host_arena: &mut AstArena,
        search_paths: &[PathBuf],
        registry: &mut ModuleRegistry,
    ) -> Result<ScopeId, Vec<Diagnostic>> {
        // Read the source file
        let source = std::fs::read_to_string(path).map_err(|e| {
            vec![Diagnostic::error(format!(
                "Failed to read module '{}' from {}: {}",
                name,
                path.display(),
                e,
            ))]
        })?;

        let source_idx = host_source_map.len();
        let file_name = path.to_string_lossy().to_string();
        host_source_map.push((file_name.clone(), source.clone()));

        let options = crate::CompilerOptions {
            output_path: None,
            emit_llvm: false,
            emit_mvir: false,
            emit_mlib: true,
            quiet: true,
            search_paths: search_paths.to_vec(),
            check_only: false,
            run_after: false,
            in_memory: true,
        };

        let file_name_str = path.to_string_lossy().to_string();
        
        let mlib_bytes_opt = crate::compile(&file_name_str, &source, &options)?;
        
        let mlib_bytes = mlib_bytes_opt.ok_or_else(|| {
            vec![Diagnostic::error(format!("Failed to compile module '{}' in-memory", name))]
        })?;

        // Now deserialize it just like load_mlib does!
        let mut cursor = std::io::Cursor::new(mlib_bytes);
        
        let (mut module_arena, module_root_items, _source) = luna_llib::MlibReader::read_ast_interface(&mut cursor)
            .map_err(|e| vec![Diagnostic::error(format!("Failed to read compiled .mlib '{}': {:?}", name, e))])?
            .ok_or_else(|| vec![Diagnostic::error(format!("No AstInterface section in compiled .mlib '{}'", name))])?;

        // Generate a new FileId for this loaded module.
        let new_file_id = luna_common::ids::FileId(host_arena.exprs.len() as u32 + 1000); // Placeholder unique FileId

        // Prepare offsets for relocation
        let expr_offset = host_arena.exprs.len() as u32;
        let stmt_offset = host_arena.stmts.len() as u32;
        let decl_offset = host_arena.decls.len() as u32;
        let type_offset = host_arena.types.len() as u32;
        let pat_offset = host_arena.pats.len() as u32;

        let relocator = luna_ast::relocator::AstRelocator::new(
            expr_offset,
            stmt_offset,
            decl_offset,
            type_offset,
            pat_offset,
            new_file_id,
        );

        relocator.relocate_arena(&mut module_arena);

        // Append relocated AST nodes to the host arena
        host_arena.exprs.extend(module_arena.exprs);
        host_arena.stmts.extend(module_arena.stmts);
        host_arena.decls.extend(module_arena.decls);
        host_arena.types.extend(module_arena.types);
        host_arena.pats.extend(module_arena.pats);

        // Build symbol table mapping for the newly injected declarations
        let module_scope = host_ctx.symbol_table.create_scope(luna_semantic::symbol::ScopeKind::Module, Some(luna_semantic::ScopeId(0)));
        
        let mut imported_items = Vec::new();
        for item in module_root_items {
            if let luna_ast::Item::Decl(d_id) = item {
                imported_items.push(luna_ast::Item::Decl(luna_ast::DeclId(d_id.0 + decl_offset)));
            }
        }

        // Resolve nested imports (e.g. alloc.ms depends on core)
        // This ensures cross-module references like core::Option are available.
        let nested_ns = Self::resolve_nested_imports(
            &imported_items,
            &_source,
            host_ctx,
            host_source_map,
            host_arena,
            search_paths,
            registry,
        );

        // Run Resolver with module provider for nested imports
        if nested_ns.scopes.is_empty() {
            let mut resolver = luna_semantic::resolver::Resolver::new(
                host_ctx,
                host_arena,
                &_source,
            );
            resolver.set_current_scope(module_scope);
            resolver.resolve_items(&imported_items);
        } else {
            let mut resolver = luna_semantic::resolver::Resolver::new_with_modules(
                host_ctx,
                host_arena,
                &_source,
                &nested_ns,
            );
            resolver.set_current_scope(module_scope);
            resolver.resolve_items(&imported_items);
        }

        // Run TypeChecker on imported items using the module's source string
        let mut typechecker = luna_semantic::typechecker::TypeChecker::new(
            host_ctx,
            host_arena,
            &_source,
        );
        typechecker.typecheck_items(&imported_items);

        Ok(module_scope)
    }

    /// Load a `.mlib` compiled artifact.
    fn load_mlib(
        path: &Path,
        name: &ModuleName,
        host_ctx: &mut SemanticContext,
        host_source_map: &mut Vec<(String, String)>,
        host_arena: &mut AstArena,
        search_paths: &[PathBuf],
        registry: &mut ModuleRegistry,
    ) -> Result<ScopeId, Vec<Diagnostic>> {
        let mut file = std::fs::File::open(path).map_err(|e| {
            vec![Diagnostic::error(format!(
                "Failed to open .mlib module '{}' at {}: {}",
                name,
                path.display(),
                e
            ))]
        })?;

        let (mut module_arena, module_root_items, _source) = luna_llib::MlibReader::read_ast_interface(&mut file)
            .map_err(|e| vec![Diagnostic::error(format!("Failed to read .mlib '{}': {:?}", name, e))])?
            .ok_or_else(|| vec![Diagnostic::error(format!("No AstInterface section in .mlib '{}'", name))])?;

        // Generate a new FileId for this loaded module.
        let new_file_id = luna_common::ids::FileId(host_arena.exprs.len() as u32 + 1000);

        // Prepare offsets for relocation
        let expr_offset = host_arena.exprs.len() as u32;
        let stmt_offset = host_arena.stmts.len() as u32;
        let decl_offset = host_arena.decls.len() as u32;
        let type_offset = host_arena.types.len() as u32;
        let pat_offset = host_arena.pats.len() as u32;

        let relocator = luna_ast::relocator::AstRelocator::new(
            expr_offset,
            stmt_offset,
            decl_offset,
            type_offset,
            pat_offset,
            new_file_id,
        );

        relocator.relocate_arena(&mut module_arena);

        // Append relocated AST nodes to the host arena
        host_arena.exprs.extend(module_arena.exprs);
        host_arena.stmts.extend(module_arena.stmts);
        host_arena.decls.extend(module_arena.decls);
        host_arena.types.extend(module_arena.types);
        host_arena.pats.extend(module_arena.pats);

        // Build symbol table mapping for the newly injected declarations
        let module_scope = host_ctx.symbol_table.create_scope(luna_semantic::symbol::ScopeKind::Module, Some(luna_semantic::ScopeId(0)));
        
        let mut imported_items = Vec::new();
        for item in module_root_items {
            if let luna_ast::Item::Decl(d_id) = item {
                imported_items.push(luna_ast::Item::Decl(luna_ast::DeclId(d_id.0 + decl_offset)));
            }
        }

        // Resolve nested imports (e.g. alloc.ms depends on core)
        let nested_ns = Self::resolve_nested_imports(
            &imported_items,
            &_source,
            host_ctx,
            host_source_map,
            host_arena,
            search_paths,
            registry,
        );

        // Run Resolver with module provider for nested imports
        if nested_ns.scopes.is_empty() {
            let mut resolver = luna_semantic::resolver::Resolver::new(
                host_ctx,
                host_arena,
                &_source,
            );
            resolver.set_current_scope(module_scope);
            resolver.resolve_items(&imported_items);
        } else {
            let mut resolver = luna_semantic::resolver::Resolver::new_with_modules(
                host_ctx,
                host_arena,
                &_source,
                &nested_ns,
            );
            resolver.set_current_scope(module_scope);
            resolver.resolve_items(&imported_items);
        }

        // Run TypeChecker on imported items using the module's source string
        let mut typechecker = luna_semantic::typechecker::TypeChecker::new(
            host_ctx,
            host_arena,
            &_source,
        );
        typechecker.typecheck_items(&imported_items);

        Ok(module_scope)
    }
}
