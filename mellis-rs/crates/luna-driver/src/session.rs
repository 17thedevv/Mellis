use std::path::PathBuf;
use luna_ast::AstArena;
use luna_common::CompilerSession;
use luna_semantic::symbol::ProviderId;
use crate::sysroot::Sysroot;
use crate::registry::ModuleRegistry;
use crate::discovery::ExternalComponentDiscovery;
use crate::error::{BootstrapError, ExternalComponentError};
use crate::resolution_context::ProviderResolutionContext;
use crate::external::ExternalComponentLoader;

pub struct DriverSession<'a> {
    pub sysroot: Sysroot,
    pub registry: ModuleRegistry,
    pub compiler_session: &'a mut CompilerSession,
    pub search_paths: Vec<PathBuf>,
    pub collected_objects: Vec<PathBuf>,
}

impl<'a> DriverSession<'a> {
    pub fn new(
        sysroot: Sysroot,
        compiler_session: &'a mut CompilerSession,
        search_paths: &[String],
    ) -> Self {
        Self {
            sysroot,
            registry: ModuleRegistry::new(),
            compiler_session,
            search_paths: search_paths.iter().map(PathBuf::from).collect(),
            collected_objects: Vec::new(),
        }
    }

    pub fn add_collected_object(&mut self, path: PathBuf) {
        if !self.collected_objects.contains(&path) {
            self.collected_objects.push(path);
        }
    }

    /// Bootstraps required external language contracts.
    pub fn bootstrap_lang_contracts(
        &mut self,
        arena: &mut AstArena,
        
    ) -> Result<(), BootstrapError> {
        self.sysroot.validate().map_err(BootstrapError::Sysroot)?;

        let manifest = crate::lang_contracts::LangContractManifest::canonical();

        // Load all contracts in deterministic order.
        // If they depend on prerequisites (like core/panic), `load_package` will resolve them using standard machinery!
        for entry in manifest.contracts {
            let provider_id = match self.load_package(entry.provider_id, arena, ProviderResolutionContext::Bootstrap) {
                Ok(id) => id,
                Err(err) => {
                    for d in err.into_diagnostics() {
                        eprintln!("{:?}", d);
                    }
                    return Err(BootstrapError::MissingRequiredComponent(entry.provider_id.to_string()));
                }
            };
            
            // Note: controlled auto-visibility will be implemented in registry/importer side.
            // For now, we just ensure they are loaded and marked as external providers.
            self.registry.external_providers.insert(entry.provider_id.to_string());
        }

        Ok(())
    }

    /// Resolves an external package component (e.g. for `import <pkg>;`).
    /// Guarantees load-once semantics: returns existing ProviderId if already loaded.
    pub fn load_package(
        &mut self,
        name: &str,
        arena: &mut AstArena,
        context: ProviderResolutionContext,
    ) -> Result<ProviderId, ExternalComponentError> {
        if let Some(provider) = self.sysroot.manifest().find_provider(name) {
            if provider.visibility == crate::sysroot_manifest::ProviderVisibility::Internal
                && !context.can_access_internal()
            {
                return Err(ExternalComponentError::NotFound {
                    name: name.to_string(),
                    searched_dir: self.sysroot.external_dir().to_path_buf(),
                });
            }
        }

        if let Some(&id) = self.registry.providers.get(name) {
            return Ok(id);
        }

        let descriptor = {
            let sysroot_manifest = match self.sysroot.validate() {
                Ok(_) => Some(self.sysroot.manifest()),
                Err(_) => None,
            };

            let mut desc = None;
            for path in &self.search_paths {
                // If it's a sysroot search path or normal path, try to use manifest if available.
                // Note: To support purely custom `-L` paths (non-sysroot), we might need a fallback,
                // but for now we follow the strict Phase 2.5 blueprint.
                if let Some(manifest) = sysroot_manifest {
                    if let Ok(d) = ExternalComponentDiscovery::discover(path, name, manifest, context) {
                        desc = Some(d);
                        break;
                    }
                }
            }
            if desc.is_none() && sysroot_manifest.is_some() {
                desc = ExternalComponentDiscovery::discover(self.sysroot.external_dir(), name, sysroot_manifest.unwrap(), context).ok();
            }
            desc
        };

        let descriptor = descriptor.ok_or_else(|| ExternalComponentError::NotFound {
            name: name.to_string(),
            searched_dir: self.sysroot.external_dir().to_path_buf(),
        })?;

        ExternalComponentLoader::load_component(&descriptor, arena, self)
    }
}
