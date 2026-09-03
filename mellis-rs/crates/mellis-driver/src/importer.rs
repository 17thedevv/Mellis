use mellis_ast::{AstArena, Item, Decl};
use mellis_common::{CompilerSession, Diagnostic};
use std::path::Path;
use crate::registry::{ModuleRegistry, ProviderInterface};
use mellis_lexer::Lexer;
use mellis_parser::Parser;
use mellis_semantic::{SemanticContext, Resolver, TypeChecker};

#[allow(dead_code)]
pub fn resolve_imports(
    items: &[Item],
    arena: &mut AstArena,
    source: &mut String,
    search_paths: &[String],
    registry: &mut ModuleRegistry,
    session: &mut CompilerSession,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Collect imports from the current items
    let mut imports = Vec::new();
    for item in items {
        if let Item::Decl(decl_id) = item {
            if let Decl::Import { annotations: _, name: name_span, kind: _, visibility: _ } = &arena.decls[decl_id.0 as usize] {
                let mut name_str = &source[name_span.start as usize .. name_span.end as usize];
                if name_str.starts_with('"') && name_str.ends_with('"') {
                    name_str = &name_str[1..name_str.len() - 1];
                }
                imports.push((name_str.to_string(), *name_span));
            }
        }
    }

    for (name, span) in imports {
        if registry.providers.contains_key(&name) {
            continue; // Already loaded
        }
        if registry.is_loading(&name) {
            diagnostics.push(Diagnostic::error(format!("Cyclic module dependency detected involving '{}'", name)).with_span(span));
            continue;
        }

        registry.start_loading(&name);

        // Try to find .mlib or .ms in search paths
        let mut found_path = None;
        let mut is_mlib = false;
        for sp in search_paths {
            let mlib_path = Path::new(sp).join(format!("{}.mlib", name));
            let ms_path = Path::new(sp).join(format!("{}.ms", name));
            println!("check debug: path checked = {}", ms_path.display());
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
                        registry.finish_loading();
                        continue;
                    }
                };
                let file_id = session.source_manager.add_file(path.to_string_lossy().to_string(), input.clone());
                
                let lexer = Lexer::new(&input, file_id);
                let mut provider_arena = AstArena::new();
                let mut parser = Parser::new(lexer, &mut provider_arena, file_id);
                
                let parse_res = parser.parse_file();
                if !parser.diagnostics.is_empty() {
                    diagnostics.extend(parser.diagnostics);
                    registry.finish_loading();
                    continue;
                }
                let Ok(provider_items) = parse_res else {
                    diagnostics.push(Diagnostic::error(format!("Failed to parse provider '{}'", path.display())));
                    registry.finish_loading();
                    continue;
                };
                
                if let Err(mut inner_diags) = resolve_imports(&provider_items, &mut provider_arena, source, search_paths, registry, session) {
                    diagnostics.append(&mut inner_diags);
                }
                
                let mut semantic_ctx = SemanticContext::new();
                registry.inject_into_ctx(&mut semantic_ctx);
                Resolver::new(&mut semantic_ctx, &provider_arena, &input).resolve_items(&provider_items);
                TypeChecker::new(&mut semantic_ctx, &provider_arena, &input).typecheck_items(&provider_items);
                
                if !semantic_ctx.diagnostics.is_empty() {
                    diagnostics.extend(semantic_ctx.diagnostics);
                    registry.finish_loading();
                    continue;
                }
                
                let provider_id = registry.allocate_id();
                
                let offset = source.len() as u32;
                source.push('\n');
                source.push_str(&input);
                
                let mut relocator = mellis_ast::relocator::AstRelocator::new(
                    arena.exprs.len() as u32,
                    arena.stmts.len() as u32,
                    arena.decls.len() as u32,
                    arena.types.len() as u32,
                    arena.pats.len() as u32,
                    file_id,
                    offset + 1,
                );
                
                relocator.relocate_arena(&mut provider_arena);
                
                let mut interface = ModuleRegistry::extract_interface_from_ctx(name.clone(), provider_id, &semantic_ctx);
                
                fn shift_ns(ns: &mut crate::registry::ExternalSymbol, relocator: &mellis_ast::relocator::AstRelocator) {
                    if let Some(did) = ns.sym.decl_id {
                        ns.sym.decl_id = Some(relocator.shift_decl_id(did));
                    }
                    for child in ns.children.values_mut() {
                        shift_ns(child, relocator);
                    }
                }
                
                for ns in interface.exported_symbols.values_mut() {
                    shift_ns(ns, &relocator);
                }
                
                registry.register(name.clone(), interface);
                
                arena.exprs.append(&mut provider_arena.exprs);
                arena.stmts.append(&mut provider_arena.stmts);
                arena.decls.append(&mut provider_arena.decls);
                arena.types.append(&mut provider_arena.types);
                arena.pats.append(&mut provider_arena.pats);
            }
        } else {
            diagnostics.push(Diagnostic::error(format!("Could not resolve module provider '{}'", name)).with_span(span));
        }
        registry.finish_loading();
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
