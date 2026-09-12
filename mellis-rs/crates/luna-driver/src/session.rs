use std::path::PathBuf;
use luna_ast::AstArena;
use luna_common::CompilerSession;
use luna_semantic::symbol::ProviderId;
use crate::sysroot::Sysroot;
use crate::registry::ModuleRegistry;
use crate::discovery::ExternalComponentDiscovery;
use crate::external::ExternalComponentLoader;
use crate::error::{BootstrapError, ExternalComponentError};

pub struct DriverSession<'a> {
    pub sysroot: Sysroot,
    pub registry: ModuleRegistry,
    pub compiler_session: &'a mut CompilerSession,
    pub search_paths: Vec<PathBuf>,
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
        }
    }

    /// Bootstraps required external component "core" exactly once.
    pub fn bootstrap_core(
        &mut self,
        arena: &mut AstArena,
        
    ) -> Result<ProviderId, BootstrapError> {
        // If core is somehow already registered, reuse it directly
        if let Some(&id) = self.registry.providers.get("core") {
            self.registry.external_providers.insert("core".to_string());
            return Ok(id);
        }

        self.sysroot.validate().map_err(BootstrapError::Sysroot)?;

        let descriptor = ExternalComponentDiscovery::discover(self.sysroot.external_dir(), "core")
            .map_err(|_| BootstrapError::MissingRequiredComponent("core".to_string()))?;

        ExternalComponentLoader::load_component(
            &descriptor,
            arena,
            self,
        ).map_err(BootstrapError::Component)
    }

    /// Resolves an external package component (e.g. for `import <pkg>;`).
    /// Guarantees load-once semantics: returns existing ProviderId if already loaded.
    pub fn load_package(
        &mut self,
        name: &str,
        arena: &mut AstArena,
        
    ) -> Result<ProviderId, ExternalComponentError> {
        if let Some(&id) = self.registry.providers.get(name) {
            return Ok(id);
        }

        let descriptor = if self.sysroot.validate().is_ok() {
            ExternalComponentDiscovery::discover(self.sysroot.external_dir(), name).ok()
        } else {
            None
        };

        let descriptor = descriptor.ok_or_else(|| ExternalComponentError::NotFound {
            name: name.to_string(),
            searched_dir: self.sysroot.external_dir().to_path_buf(),
        })?;

        ExternalComponentLoader::load_component(&descriptor, arena, self)
    }
}
