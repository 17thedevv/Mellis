use std::path::{Path, PathBuf};
use crate::error::ExternalComponentError;
use crate::sysroot_manifest::{SysrootManifest, ProviderVisibility};
use crate::resolution_context::ProviderResolutionContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentFormat {
    Mlib,
    Llib,
    Source,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalComponentDescriptor {
    pub name: String,
    pub root_dir: PathBuf,
    pub entry_file: PathBuf,
    pub format: ComponentFormat,
}

pub struct ExternalComponentDiscovery;

impl ExternalComponentDiscovery {
    pub fn discover(
        external_dir: &Path,
        name: &str,
        manifest: &SysrootManifest,
        context: ProviderResolutionContext,
    ) -> Result<ExternalComponentDescriptor, ExternalComponentError> {
        let provider = manifest.find_provider(name).ok_or_else(|| {
            ExternalComponentError::NotFound { name: name.to_string(), searched_dir: external_dir.to_path_buf() }
        })?;

        if provider.visibility == ProviderVisibility::Internal && !context.can_access_internal() {
            return Err(ExternalComponentError::NotFound { name: name.to_string(), searched_dir: external_dir.to_path_buf() });
        }

        let path_str = provider.path.as_str();

        let llib_path = external_dir.join(format!("{}.llib", path_str));
        if llib_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: llib_path,
                format: ComponentFormat::Llib,
            });
        }

        let ln_path = external_dir.join(format!("{}.ln", path_str));
        if ln_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: ln_path,
                format: ComponentFormat::Source,
            });
        }

        let pkg_root = external_dir.join(path_str);
        let pkg_ln_entry = pkg_root.join("package.ln");
        if pkg_ln_entry.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: pkg_root,
                entry_file: pkg_ln_entry,
                format: ComponentFormat::Package,
            });
        }

        let mlib_path = external_dir.join(format!("{}.mlib", path_str));
        if mlib_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: mlib_path,
                format: ComponentFormat::Llib,
            });
        }

        let ms_path = external_dir.join(format!("{}.ms", path_str));
        if ms_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: ms_path,
                format: ComponentFormat::Source,
            });
        }

        let pkg_ms_entry = pkg_root.join("package.ms");
        if pkg_ms_entry.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: pkg_root,
                entry_file: pkg_ms_entry,
                format: ComponentFormat::Package,
            });
        }

        Err(ExternalComponentError::NotFound {
            name: name.to_string(),
            searched_dir: external_dir.to_path_buf(),
        })
    }
}
