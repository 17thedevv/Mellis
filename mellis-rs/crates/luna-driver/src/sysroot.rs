use std::path::{Path, PathBuf};
use crate::error::SysrootError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sysroot {
    root: PathBuf,
    external_dir: PathBuf,
}

impl Sysroot {
    pub fn from_root(root: PathBuf) -> Self {
        let external_dir = if root.join("libs").join("external").exists() {
            root.join("libs").join("external")
        } else if root.ends_with("external") {
            root.clone()
        } else {
            root.join("libs").join("external")
        };
        Self { root, external_dir }
    }

    pub fn new(root: PathBuf, external_dir: PathBuf) -> Self {
        Self { root, external_dir }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn external_dir(&self) -> &Path {
        &self.external_dir
    }

    pub fn validate(&self) -> Result<(), SysrootError> {
        if !self.external_dir.exists() {
            return Err(SysrootError::ExternalRootMissing(self.external_dir.clone()));
        }
        Ok(())
    }

    /// Production sysroot discovery:
    /// 1. Explicit argument (`--sysroot`)
    /// 2. `MELLIS_SYSROOT` environment variable
    /// 3. Executable-relative path (`current_exe()` ancestor containing `libs/external`)
    pub fn discover(explicit: Option<&str>) -> Result<Self, SysrootError> {
        let mut searched_candidates = Vec::new();

        // 1. Explicit option
        if let Some(exp) = explicit {
            let root = PathBuf::from(exp);
            if !root.exists() {
                return Err(SysrootError::SpecifiedPathNotFound(root));
            }
            return Ok(Self::from_root(root));
        }

        // 2. Environment variable
        if let Ok(env_val) = std::env::var("LUNA_SYSROOT").or_else(|_| std::env::var("MELLIS_SYSROOT")) {
            if !env_val.trim().is_empty() {
                let root = PathBuf::from(env_val.trim());
                if !root.exists() {
                    return Err(SysrootError::EnvVarPathNotFound(root));
                }
                return Ok(Self::from_root(root));
            }
        }

        // 3. Executable-relative discovery
        if let Ok(mut exe) = std::env::current_exe() {
            // <dir>/exe -> <dir>
            exe.pop();
            searched_candidates.push(exe.clone());
            if exe.join("libs").join("external").exists() {
                return Ok(Self::from_root(exe));
            }

            // e.g., <install_root>/bin -> <install_root>
            if exe.pop() {
                searched_candidates.push(exe.clone());
                if exe.join("libs").join("external").exists() {
                    return Ok(Self::from_root(exe));
                }

                // e.g., <repo_root>/target/debug -> <repo_root>
                if exe.pop() {
                    searched_candidates.push(exe.clone());
                    if exe.join("libs").join("external").exists() {
                        return Ok(Self::from_root(exe));
                    }
                }
            }
        }

        Err(SysrootError::DiscoveryFailed {
            searched: searched_candidates,
        })
    }

    /// Test/dev helper: tries production discovery first, then falls back to CWD ancestor walking
    pub fn discover_for_test() -> Result<Self, SysrootError> {
        if let Ok(sysroot) = Self::discover(None) {
            if sysroot.validate().is_ok() {
                return Ok(sysroot);
            }
        }

        if let Ok(mut cwd) = std::env::current_dir() {
            for _ in 0..6 {
                let ext = cwd.join("libs").join("external");
                if ext.exists() {
                    return Ok(Self::from_root(cwd));
                }
                if !cwd.pop() {
                    break;
                }
            }
        }

        Self::discover(None)
    }
}
