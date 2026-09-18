use luna_ast::{AstArena, Item, Decl, ImportKind};
use luna_common::Diagnostic;
use std::path::Path;
use crate::registry::ModuleRegistry;
use crate::session::DriverSession;
use luna_lexer::Lexer;
use luna_parser::Parser;
use luna_semantic::{SemanticContext, Resolver, TypeChecker};

pub fn get_imports(
    items: &[Item],
    arena: &AstArena,
    session: &DriverSession,
) -> Vec<(String, luna_common::ids::Span, ImportKind)> {
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
    imports
}

pub fn resolve_collected_imports(
    imports: Vec<(String, luna_common::ids::Span, ImportKind)>,
    arena: &mut AstArena,
    session: &mut DriverSession,
    context: crate::resolution_context::ProviderResolutionContext,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();



    for (name, span, kind) in imports {
        if kind == ImportKind::External {
            // External package import: import <pkg>; -> delegates to session.load_package
            if session.registry.is_loading(&name) {
                diagnostics.push(Diagnostic::error(format!("Cyclic module dependency detected involving '{}'", name)).with_span(span));
                continue;
            }
            match session.load_package(&name, arena, context) {
                Ok(_) => continue,
                Err(err) => {
                    for d in err.into_diagnostics() {
                        diagnostics.push(d.with_span(span));
                    }
                    continue;
                }
            }
        } else {
            // Local module import: import "module"; -> searches relative to the importing file directory
            if session.registry.local_providers.contains(&name) {
                continue;
            }
            if session.registry.is_loading(&name) {
                diagnostics.push(Diagnostic::error(format!("Cyclic module dependency detected involving '{}'", name)).with_span(span));
                continue;
            }

            let mut found_path = None;
            let mut is_binary = false;
            
            // Get the directory of the current file
            if let Some(file_info) = session.compiler_session.source_manager.get_file(span.file_id) {
                let file_path = Path::new(&file_info.name);
                if let Some(parent_dir) = file_path.parent() {
                        let base_dir = parent_dir;
                        let llib_path = base_dir.join(format!("{}.llib", name));
                        let mlib_path = base_dir.join(format!("{}.mlib", name));
                        let ln_path = base_dir.join(format!("{}.ln", name));
                        let ms_path = base_dir.join(format!("{}.ms", name));
                        
                        // Canonical-first (COMPAT-PRECEDENCE-01): `.llib` > `.ln` >
                        // legacy `.mlib` > legacy `.ms`. A legacy artifact must never
                        // shadow canonical source. Mirrors the discovery precedence.
                        if llib_path.exists() {
                            found_path = Some(llib_path);
                            is_binary = true;
                        } else if ln_path.exists() {
                            found_path = Some(ln_path);
                        } else if mlib_path.exists() {
                            found_path = Some(mlib_path);
                            is_binary = true;
                        } else if ms_path.exists() {
                            found_path = Some(ms_path);
                        }
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
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

pub fn resolve_imports(
    items: &[Item],
    arena: &mut AstArena,
    session: &mut DriverSession,
    context: crate::resolution_context::ProviderResolutionContext,
) -> Result<(), Vec<Diagnostic>> {
    let imports = get_imports(items, arena, session);
    resolve_collected_imports(imports, arena, session, context)
}
