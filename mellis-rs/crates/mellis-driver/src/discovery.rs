use std::path::{Path, PathBuf};
use crate::error::ExternalComponentError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentFormat {
    Mlib,
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
    pub fn discover(external_dir: &Path, name: &str) -> Result<ExternalComponentDescriptor, ExternalComponentError> {
        // 1. Check for precompiled library: <external_dir>/<name>.mlib (per Rule 6)
        let mlib_path = external_dir.join(format!("{}.mlib", name));
        if mlib_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: mlib_path,
                format: ComponentFormat::Mlib,
            });
        }

        // 2. Check for single source file: <external_dir>/<name>.ms
        let ms_path = external_dir.join(format!("{}.ms", name));
        if ms_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: ms_path,
                format: ComponentFormat::Source,
            });
        }

        // 3. Check for package directory: <external_dir>/<name>/package.ms
        let pkg_root = external_dir.join(name);
        let pkg_entry = pkg_root.join("package.ms");
        if pkg_entry.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: pkg_root,
                entry_file: pkg_entry,
                format: ComponentFormat::Package,
            });
        }

        Err(ExternalComponentError::NotFound {
            name: name.to_string(),
            searched_dir: external_dir.to_path_buf(),
        })
    }
}
