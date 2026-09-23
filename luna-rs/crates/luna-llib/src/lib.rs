pub mod format;
pub mod ir;
pub mod metadata;
pub mod writer;
pub mod reader;

pub use format::*;
pub use ir::*;
pub use metadata::*;
pub use writer::*;
pub use reader::*;

#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub expected_compiler_version: String,
    pub expected_target: String,
    pub expected_source_fingerprint: Option<format::Fingerprint>,
    pub expected_dependencies: std::collections::HashMap<String, format::Fingerprint>,
}

pub fn validate_artifact(manifest: &crate::format::Manifest, ctx: &ValidationContext) -> Result<(), String> {
    if manifest.provenance.compiler_version != ctx.expected_compiler_version {
        return Err(format!("compiler version mismatch: expected {}, got {}", ctx.expected_compiler_version, manifest.provenance.compiler_version));
    }
    if manifest.target.target_triple != ctx.expected_target {
        return Err(format!("target triple mismatch: expected {}, got {}", ctx.expected_target, manifest.target.target_triple));
    }
    if let Some(expected_source) = ctx.expected_source_fingerprint {
        if manifest.provenance.source_fingerprint != expected_source {
            return Err(format!("source fingerprint mismatch"));
        }
    }
    for dep in &manifest.dependencies.deps {
        if let Some(expected_dep_fingerprint) = ctx.expected_dependencies.get(&dep.provider_name) {
            if &dep.interface_fingerprint != expected_dep_fingerprint {
                return Err(format!("dependency interface fingerprint mismatch for provider {}", dep.provider_name));
            }
        }
    }
    Ok(())
}
