use mellis_ast::{AstArena, Item, Decl, ImportKind};
use mellis_common::Diagnostic;
use std::path::Path;
use crate::registry::ModuleRegistry;
use crate::session::DriverSession;
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::{SemanticContext, Resolver, TypeChecker};

pub fn resolve_imports(
    items: &[Item],
    arena: &mut AstArena,
    
    session: &mut DriverSession,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Collect imports from the current items
    let mut imports = Vec::new();
    for item in items {
        if let Item::Decl(decl_id) = item {
            if let Decl::Import { annotations: _, name: name_span, kind, visibility: _ } = &arena.decls[decl_id.0 as usize] {
                let file_info = session.compiler_session.source_manager.get_file(name_span.file_id).unwrap();
                let mut name_str = &file_info.source[name_span.start as usize .. name_span.end as usize];
                if name_str.starts_with('"') && name_str.ends_with('"') {
                    name_str = &name_str[1..name_str.len() - 1];
                }
                imports.push((name_str.to_string(), *name_span, *kind));
            }
        }
    }

    for (name, span, kind) in imports {
        if kind == ImportKind::External {
            // External package import: import <pkg>; -> delegates to session.load_package
            if session.registry.external_providers.contains(&name) {
                continue;
            }
            if session.registry.is_loading(&name) {
                diagnostics.push(Diagnostic::error(format!("Cyclic module dependency detected involving '{}'", name)).with_span(span));
                continue;
            }
            match session.load_package(&name, arena) {
                Ok(_) => continue,
                Err(err) => {
                    for d in err.into_diagnostics() {
                        diagnostics.push(d.with_span(span));
                    }
                    continue;
                }
            }
        } else {
            // Local module import: import "module"; -> searches in session.search_paths ONLY
            if session.registry.local_providers.contains(&name) {
                continue;
            }
            if session.registry.is_loading(&name) {
                diagnostics.push(Diagnostic::error(format!("Cyclic module dependency detected involving '{}'", name)).with_span(span));
                continue;
            }

            session.registry.start_loading(&name);

        // Try to find .mlib or .ms in search paths
        let mut found_path = None;
        let mut is_mlib = false;
        for sp in &session.search_paths {
            let mlib_path = Path::new(sp).join(format!("{}.mlib", name));
            let ms_path = Path::new(sp).join(format!("{}.ms", name));
            if mlib_path.exists() {
                found_path = Some(mlib_path);
                is_mlib = true;
                break;
            }
            if ms_path.exists() {
                found_path = Some(ms_path);
                break;
            }
        }

        if let Some(path) = found_path {
            if is_mlib {
                diagnostics.push(Diagnostic::error(format!("Loading .mlib is not yet fully implemented for '{}'", name)).with_span(span));
            } else {
                let input = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        diagnostics.push(Diagnostic::error(format!("failed to read module file `{}`: {}", path.display(), e)).with_span(span));
                        session.registry.finish_loading();
                        continue;
                    }
                };
                let file_id = session.compiler_session.source_manager.add_file(path.to_string_lossy().to_string(), input.clone());
                
                let lexer = Lexer::new(&input, file_id);
                let mut provider_arena = AstArena::new();
                let mut parser = Parser::new(lexer, &mut provider_arena, file_id);
                
                let parse_res = parser.parse_file();
                if !parser.diagnostics.is_empty() {
                    diagnostics.extend(parser.diagnostics);
                    session.registry.finish_loading();
                    continue;
                }
                let Ok(provider_items) = parse_res else {
                    diagnostics.push(Diagnostic::error(format!("Failed to parse provider '{}'", path.display())));
                    session.registry.finish_loading();
                    continue;
                };
                
                if let Err(mut inner_diags) = resolve_imports(&provider_items, &mut provider_arena, session) {
                    diagnostics.append(&mut inner_diags);
                }
                
                
                let relocator = mellis_ast::relocator::AstRelocator::new(
                    arena.exprs.len() as u32,
                    arena.stmts.len() as u32,
                    arena.decls.len() as u32,
                    arena.types.len() as u32,
                    arena.pats.len() as u32,
                    file_id,
                    
                );
                
                relocator.relocate_arena(&mut provider_arena);
                
                let mut shifted_provider_items = provider_items.clone();
                for item in &mut shifted_provider_items {
                    if let Item::Decl(decl_id) = item {
                        *decl_id = relocator.shift_decl_id(*decl_id);
                    }
                }
                
                arena.exprs.append(&mut provider_arena.exprs);
                arena.stmts.append(&mut provider_arena.stmts);
                arena.decls.append(&mut provider_arena.decls);
                arena.types.append(&mut provider_arena.types);
                arena.pats.append(&mut provider_arena.pats);
                
                let provider_id = session.registry.allocate_id();
                let mut semantic_ctx = SemanticContext::new();
                semantic_ctx.current_provider = Some(provider_id);
                semantic_ctx.allow_internal_lang_items = session.compiler_session.allow_internal_lang_items;
                session.registry.inject_into_ctx(&mut semantic_ctx);
                Resolver::new(&mut semantic_ctx, arena, &session.compiler_session.source_manager).resolve_items(&shifted_provider_items);
                TypeChecker::new(&mut semantic_ctx, arena, &session.compiler_session.source_manager).typecheck_items(&shifted_provider_items);
                
                if !semantic_ctx.diagnostics.is_empty() {
                    diagnostics.extend(semantic_ctx.diagnostics);
                    session.registry.finish_loading();
                    continue;
                }
                
                let interface = ModuleRegistry::extract_interface_from_ctx(name.clone(), provider_id, &semantic_ctx);
                session.registry.register_local(name.clone(), interface);
            }
        } else {
            diagnostics.push(Diagnostic::error(format!("Could not resolve module provider '{}'", name)).with_span(span));
        }
        session.registry.finish_loading();
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
