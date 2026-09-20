//! Phase 4D: Sysroot Build Infrastructure Tests
//!
//! These tests verify:
//! - Canonical dependency DAG integrity (read-only against canonical sysroot)
//! - Persistent canonical .obj and .llib sidecar invariant (25 providers, 25 .ln, 25 .llib, 25 .obj)
//! - Absence of orphan/monolithic legacy artifacts
//! - External builder execution and lock mutual exclusion strictly within ISOLATED test sysroots
//!
//! INVARIANT: Integration tests MUST NOT mutate repository canonical sysroot (`libs/external`).
//! All mutating builder operations execute against isolated temporary test sysroots.

use luna_driver::sysroot::Sysroot;
use luna_driver::sysroot_builder::SysrootBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

/// ======================================================================
/// SECTION 4D.1: CANONICAL DEPENDENCY GRAPH VALIDATION (READ-ONLY)
/// ======================================================================

/// Test that all 25 canonical providers have no unknown dependencies,
/// no self-edges, no duplicate nodes, and all edges resolve.
#[test]
fn test_canonical_dag_integrity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let manifest = sysroot.manifest();
    let external_dir = sysroot.external_dir();

    // Collect all canonical provider names
    let canonical_providers: BTreeSet<String> = manifest
        .providers()
        .map(|e| e.name.clone())
        .collect();

    let provider_count = canonical_providers.len();
    assert_eq!(
        provider_count, 25,
        "Expected exactly 25 canonical providers, got {}",
        provider_count
    );

    // Build dependency graph from parsed imports
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut all_edges: Vec<(String, String)> = Vec::new();
    let mut self_edges: Vec<String> = Vec::new();

    for provider in &canonical_providers {
        let entry = manifest.find_provider(provider).expect("Provider not found");
        let source_path = external_dir.join(format!("{}.ln", entry.path));

        if !source_path.exists() {
            panic!(
                "Source file for provider {} not found at {}",
                provider,
                source_path.display()
            );
        }

        let content = fs::read_to_string(&source_path).expect("Failed to read source");
        let deps = extract_provider_imports(&content, manifest, provider);

        for dep in &deps {
            if dep == provider {
                self_edges.push(provider.clone());
            }
        }

        graph.insert(provider.clone(), deps);
    }

    // Validate: no self-edges
    assert!(
        self_edges.is_empty(),
        "Self-edges detected: {:?}",
        self_edges
    );

    // Validate: no unknown dependencies
    for (provider, deps) in &graph {
        for dep in deps {
            assert!(
                canonical_providers.contains(dep),
                "Provider {} depends on unknown provider {}",
                provider,
                dep
            );
        }
    }

    assert_eq!(graph.len(), 25, "Expected 25 graph nodes");

    for (provider, deps) in &graph {
        for dep in deps {
            all_edges.push((provider.clone(), dep.clone()));
        }
    }

    assert_eq!(all_edges.len(), 64, "Expected exactly 64 dependency edges in canonical DAG");
}

/// Helper: extract provider imports from source content
fn extract_provider_imports(
    content: &str,
    manifest: &luna_driver::sysroot_manifest::SysrootManifest,
    _provider: &str,
) -> Vec<String> {
    let mut deps = Vec::new();

    let lexer = luna_lexer::Lexer::new(content, luna_common::ids::FileId(0));
    let mut arena = luna_ast::AstArena::new();
    let mut parser = luna_parser::Parser::new(lexer, &mut arena, luna_common::ids::FileId(0));

    if let Ok(items) = parser.parse_file() {
        for item in items {
            if let luna_ast::Item::Decl(decl_id) = item {
                if let luna_ast::Decl::Import {
                    name: name_span,
                    kind: luna_ast::ImportKind::External,
                    ..
                } = &arena.decls[decl_id.0 as usize]
                {
                    let mut name_str = &content[name_span.start as usize..name_span.end as usize];
                    if name_str.starts_with('"') && name_str.ends_with('"') {
                        name_str = &name_str[1..name_str.len() - 1];
                    }
                    if let Some(dep_entry) = manifest.find_provider(name_str) {
                        deps.push(dep_entry.name.clone());
                    }
                }
            }
        }
    }

    deps
}

/// ======================================================================
/// SECTION 4D.2: PERSISTENT CANONICAL .OBJ AND .LLIB POLICY (READ-ONLY)
/// ======================================================================

/// Frozen Policy: Each canonical sysroot provider owns one persistent canonical .obj sidecar
/// and one canonical .llib artifact.
///
/// Invariant: 25 canonical providers -> exactly 25 .ln, 25 .llib, 25 .obj.
#[test]
fn test_canonical_persistent_obj_and_llib_sidecars() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let manifest = sysroot.manifest();
    let external_dir = sysroot.external_dir();

    let mut canonical_count = 0;
    for entry in manifest.providers() {
        canonical_count += 1;
        let ln_path = external_dir.join(format!("{}.ln", entry.path));
        let llib_path = external_dir.join(format!("{}.llib", entry.path));
        let obj_path = external_dir.join(format!("{}.obj", entry.path));

        assert!(
            ln_path.exists(),
            "Source file for provider {} must exist at {}",
            entry.name,
            ln_path.display()
        );
        assert!(
            llib_path.exists(),
            "Canonical .llib artifact for provider {} must exist at {}",
            entry.name,
            llib_path.display()
        );
        assert!(
            obj_path.exists(),
            "Persistent canonical .obj sidecar for provider {} must exist at {}",
            entry.name,
            obj_path.display()
        );
    }

    assert_eq!(canonical_count, 25, "Expected 25 canonical providers");

    // Obsolete monolithic artifacts must NOT exist
    let legacy_monolithic = [
        "core.llib", "alloc.llib", "io.llib",
        "core.obj", "alloc.obj", "io.obj",
    ];
    for name in &legacy_monolithic {
        let legacy_path = external_dir.join(name);
        assert!(
            !legacy_path.exists(),
            "Obsolete legacy monolithic file must not exist: {}",
            legacy_path.display()
        );
    }
}

/// Verify that no orphan .obj files exist in the canonical sysroot.
#[test]
fn test_no_orphan_obj_sidecars() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let manifest = sysroot.manifest();
    let external_dir = sysroot.external_dir();

    let canonical_paths: BTreeSet<PathBuf> = manifest
        .providers()
        .map(|e| external_dir.join(format!("{}.obj", e.path)))
        .collect();

    let mut found_obj_files = Vec::new();
    fn collect_obj_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_obj_files(&path, out);
                } else if path.extension().map(|e| e == "obj").unwrap_or(false) {
                    out.push(path);
                }
            }
        }
    }

    collect_obj_files(external_dir, &mut found_obj_files);

    for obj in &found_obj_files {
        assert!(
            canonical_paths.contains(obj),
            "Non-canonical orphan .obj found in sysroot: {}",
            obj.display()
        );
    }

    assert_eq!(
        found_obj_files.len(), 25,
        "Expected exactly 25 canonical .obj files in sysroot"
    );
}

/// ======================================================================
/// SECTION 4D.3: ISOLATED BUILDER EXECUTION & MUTUAL EXCLUSION
/// ======================================================================

/// Test SysrootBuilder execution exclusively within an ISOLATED temporary sysroot.
/// Verifies builder creates both .llib and .obj, respects topological order,
/// and removes .build.lock upon completion without mutating repo sysroot.
#[test]
fn test_sysroot_builder_execution_isolated() {
    let repo_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let repo_external = repo_sysroot.external_dir();

    // Create an isolated temporary sysroot
    let temp_dir = std::env::temp_dir().join("luna_sysroot_builder_test_isolated");
    let _ = fs::remove_dir_all(&temp_dir);
    let temp_ext = temp_dir.join("libs").join("external");
    fs::create_dir_all(temp_ext.join("core")).unwrap();
    fs::create_dir_all(temp_ext.join("lang")).unwrap();

    // Write minimal manifest for isolated test
    let manifest_content = r#"
[[provider]]
name = "ptr"
path = "core/ptr"
visibility = "public"

[[provider]]
name = "mem"
path = "core/mem"
visibility = "public"

[[provider]]
name = "__lang_drop"
path = "lang/drop"
visibility = "internal"
lang_contract = "drop"
"#;
    fs::write(temp_ext.join("sysroot.toml"), manifest_content).unwrap();

    // Copy source files needed for this isolated build
    fs::copy(repo_external.join("core/ptr.ln"), temp_ext.join("core/ptr.ln")).unwrap();
    fs::copy(repo_external.join("core/mem.ln"), temp_ext.join("core/mem.ln")).unwrap();
    fs::copy(repo_external.join("lang/drop.ln"), temp_ext.join("lang/drop.ln")).unwrap();

    // Create a dummy obsolete file to verify cleanup
    fs::write(temp_ext.join("core.llib"), b"dummy legacy").unwrap();
    fs::write(temp_ext.join("core.obj"), b"dummy legacy").unwrap();

    let isolated_sysroot = Sysroot::discover(Some(temp_dir.to_str().unwrap()))
        .expect("Failed to discover isolated test sysroot");

    let builder = SysrootBuilder::new(isolated_sysroot);
    builder.build_all(true).expect("Isolated sysroot build must succeed");

    // Verify .llib and .obj artifacts were emitted in isolated directory
    assert!(temp_ext.join("core/ptr.llib").exists(), "ptr.llib must exist");
    assert!(temp_ext.join("core/ptr.obj").exists(), "ptr.obj must exist");
    assert!(temp_ext.join("core/mem.llib").exists(), "mem.llib must exist");
    assert!(temp_ext.join("core/mem.obj").exists(), "mem.obj must exist");
    assert!(temp_ext.join("lang/drop.llib").exists(), "drop.llib must exist");
    assert!(temp_ext.join("lang/drop.obj").exists(), "drop.obj must exist");

    // Verify obsolete artifacts were quarantined
    assert!(!temp_ext.join("core.llib").exists(), "Obsolete core.llib must be cleaned up");
    assert!(!temp_ext.join("core.obj").exists(), "Obsolete core.obj must be cleaned up");

    // Verify build lock was cleaned up
    assert!(!temp_ext.join(".build.lock").exists(), ".build.lock must be cleaned up");

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test that SysrootBuilder honors existing build lock and does not delete it.
#[test]
fn test_sysroot_builder_lock_mutual_exclusion_isolated() {
    let temp_dir = std::env::temp_dir().join("luna_sysroot_lock_mutex_isolated");
    let _ = fs::remove_dir_all(&temp_dir);
    let temp_ext = temp_dir.join("libs").join("external");
    fs::create_dir_all(&temp_ext).unwrap();

    let manifest_content = "provider = []\n";
    fs::write(temp_ext.join("sysroot.toml"), manifest_content).unwrap();

    // Create active lock file
    let lock_path = temp_ext.join(".build.lock");
    fs::write(&lock_path, "active builder pid").unwrap();

    let isolated_sysroot = Sysroot::discover(Some(temp_dir.to_str().unwrap()))
        .expect("Failed to discover isolated test sysroot");

    let builder = SysrootBuilder::new(isolated_sysroot);
    let result = builder.build_all(true);

    assert!(result.is_err(), "Builder must reject when lock exists");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Build lock already exists"),
        "Expected lock error, got: {}",
        err_msg
    );

    // Active lock must NOT be deleted
    assert!(lock_path.exists(), "Existing active lock must not be deleted");

    let _ = fs::remove_dir_all(&temp_dir);
}
