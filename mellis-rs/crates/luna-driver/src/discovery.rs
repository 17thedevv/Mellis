use std::path::{Path, PathBuf};
use crate::error::ExternalComponentError;

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
    pub fn discover(external_dir: &Path, name: &str) -> Result<ExternalComponentDescriptor, ExternalComponentError> {
        // 1. Check for canonical precompiled library: <external_dir>/<name>.llib (per Rule 6)
        let llib_path = external_dir.join(format!("{}.llib", name));
        if llib_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: llib_path,
                format: ComponentFormat::Llib,
            });
        }

        // 2. Check for canonical single source file: <external_dir>/<name>.ln
        let ln_path = external_dir.join(format!("{}.ln", name));
        if ln_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: ln_path,
                format: ComponentFormat::Source,
            });
        }

        // 3. Check for canonical package directory: <external_dir>/<name>/package.ln
        let pkg_root = external_dir.join(name);
        let pkg_ln_entry = pkg_root.join("package.ln");
        if pkg_ln_entry.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: pkg_root,
                entry_file: pkg_ln_entry,
                format: ComponentFormat::Package,
            });
        }

        // 4. Check for legacy precompiled library: <external_dir>/<name>.mlib
        let mlib_path = external_dir.join(format!("{}.mlib", name));
        if mlib_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: mlib_path,
                format: ComponentFormat::Llib,
            });
        }

        // 5. Check for legacy single source file: <external_dir>/<name>.ms
        let ms_path = external_dir.join(format!("{}.ms", name));
        if ms_path.exists() {
            return Ok(ExternalComponentDescriptor {
                name: name.to_string(),
                root_dir: external_dir.to_path_buf(),
                entry_file: ms_path,
                format: ComponentFormat::Source,
            });
        }

        // 6. Check for legacy package directory: <external_dir>/<name>/package.ms
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
