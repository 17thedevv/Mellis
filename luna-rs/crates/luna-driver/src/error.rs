use std::path::PathBuf;
use luna_common::Diagnostic;

#[derive(Debug)]
pub enum SysrootError {
    SpecifiedPathNotFound(PathBuf),
    EnvVarPathNotFound(PathBuf),
    DiscoveryFailed { searched: Vec<PathBuf> },
    ExternalRootMissing(PathBuf),
    ManifestLoadFailed { path: PathBuf, error: String },
}

impl SysrootError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        let msg = match self {
            SysrootError::SpecifiedPathNotFound(p) => {
                format!("specified sysroot path `{}` does not exist", p.display())
            }
            SysrootError::EnvVarPathNotFound(p) => {
                format!("sysroot path specified by LUNA_SYSROOT `{}` does not exist", p.display())
            }
            SysrootError::DiscoveryFailed { searched } => {
                let searched_str = searched
                    .iter()
                    .map(|p| format!("  - {}", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "cannot locate Luna sysroot. Please set LUNA_SYSROOT or specify --sysroot <DIR>.\nSearched candidate locations:\n{}",
                    searched_str
                )
            }
            SysrootError::ExternalRootMissing(p) => {
                format!("sysroot external directory `{}` does not exist", p.display())
            }
            SysrootError::ManifestLoadFailed { path, error } => {
                format!("failed to load sysroot manifest `{}`: {}", path.display(), error)
            }
        };
        vec![Diagnostic::error(msg)]
    }
}

#[derive(Debug)]
pub enum ExternalComponentError {
    NotFound { name: String, searched_dir: PathBuf },
    ReadFailed { path: PathBuf, error: String },
    ParseFailed(Vec<Diagnostic>),
    ImportFailed(Vec<Diagnostic>),
    SemanticFailed(Vec<Diagnostic>),
    InvalidLibraryInterface { name: String, path: PathBuf, reason: String },
    InvalidArtifact { name: String, path: PathBuf, reason: String },
}

impl ExternalComponentError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            ExternalComponentError::NotFound { name, searched_dir } => {
                vec![Diagnostic::error(format!(
                    "external component `{}` not found in `{}`",
                    name,
                    searched_dir.display()
                ))]
            }
            ExternalComponentError::ReadFailed { path, error } => {
                vec![Diagnostic::error(format!(
                    "failed to read external component at `{}`: {}",
                    path.display(),
                    error
                ))]
            }
            ExternalComponentError::ParseFailed(diags) => diags,
            ExternalComponentError::ImportFailed(diags) => diags,
            ExternalComponentError::SemanticFailed(diags) => diags,
            ExternalComponentError::InvalidLibraryInterface { name, path, reason } => vec![Diagnostic::error(format!("Invalid binary library interface for '{}' at `{}`: {}", name, path.display(), reason))],
            ExternalComponentError::InvalidArtifact { name, path, reason } => vec![Diagnostic::error(format!("Strict Rejection: Invalid artifact for '{}' at `{}`: {}", name, path.display(), reason))],
        }
    }
}

#[derive(Debug)]
pub enum BootstrapError {
    Sysroot(SysrootError),
    Component(ExternalComponentError),
    MissingRequiredComponent(String),
}

impl From<SysrootError> for BootstrapError {
    fn from(err: SysrootError) -> Self {
        BootstrapError::Sysroot(err)
    }
}

impl From<ExternalComponentError> for BootstrapError {
    fn from(err: ExternalComponentError) -> Self {
        BootstrapError::Component(err)
    }
}

impl BootstrapError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            BootstrapError::Sysroot(err) => err.into_diagnostics(),
            BootstrapError::Component(err) => err.into_diagnostics(),
            BootstrapError::MissingRequiredComponent(name) => {
                vec![Diagnostic::error(format!(
                    "fatal error: required component `{}` could not be bootstrapped. Luna requires `{}` for language primitives and runtime items.",
                    name, name
                ))]
            }
        }
    }
}
