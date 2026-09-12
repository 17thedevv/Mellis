use luna_ast::{AstArena, Item, Decl, ImportKind};
use luna_common::Diagnostic;
use std::path::Path;
use crate::registry::ModuleRegistry;
use crate::session::DriverSession;
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::{SemanticContext, Resolver, TypeChecker};

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

            // Try to find .llib, .mlib, .ln, or .ms in search paths per Rule 6
            let mut found_path = None;
            let mut is_binary = false;
            for sp in &session.search_paths {
                let llib_path = Path::new(sp).join(format!("{}.llib", name));
                let mlib_path = Path::new(sp).join(format!("{}.mlib", name));
                let ln_path = Path::new(sp).join(format!("{}.ln", name));
                let ms_path = Path::new(sp).join(format!("{}.ms", name));
                if llib_path.exists() {
                    found_path = Some(llib_path);
                    is_binary = true;
                    break;
                }
                if mlib_path.exists() {
                    found_path = Some(mlib_path);
                    is_binary = true;
                    break;
                }
                if ln_path.exists() {
                    found_path = Some(ln_path);
                    break;
                }
                if ms_path.exists() {
                    found_path = Some(ms_path);
                    break;
                }
            }

            if let Some(path) = found_path {
                let descriptor = crate::discovery::ExternalComponentDescriptor {
                    name: name.clone(),
                    root_dir: path.parent().unwrap_or(Path::new("")).to_path_buf(),
                    entry_file: path.clone(),
                    format: if is_binary {
                        crate::discovery::ComponentFormat::Llib
                    } else {
                        crate::discovery::ComponentFormat::Source
                    },
                };
                match crate::external::ExternalComponentLoader::load_component(&descriptor, arena, session) {
                    Ok(_) => {
                        session.registry.local_providers.insert(name.clone());
                    }
                    Err(err) => {
                        for d in err.into_diagnostics() {
                            diagnostics.push(d.with_span(span));
                        }
                    }
                }
            } else {
                diagnostics.push(Diagnostic::error(format!("Could not resolve module provider '{}'", name)).with_span(span));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
