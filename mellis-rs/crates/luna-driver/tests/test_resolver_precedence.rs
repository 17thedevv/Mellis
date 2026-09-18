use luna_driver::discovery::{ComponentFormat, ExternalComponentDiscovery};
use luna_driver::resolution_context::ProviderResolutionContext;
use luna_driver::sysroot_manifest::SysrootManifest;
use std::fs;
use std::path::{Path, PathBuf};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_manifest_resolution_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn manifest_for(dir: &Path, name: &str, path: &str, visibility: &str) -> SysrootManifest {
    let manifest_path = dir.join("sysroot.toml");
    fs::write(
        &manifest_path,
        format!(
            "[[provider]]\nname = \"{name}\"\npath = \"{path}\"\nvisibility = \"{visibility}\"\n"
        ),
    )
    .unwrap();
    SysrootManifest::load_and_validate(&manifest_path).unwrap()
}

#[test]
fn manifest_maps_logical_identity_to_physical_source() {
    let ext_dir = create_temp_dir("logical_to_physical");
    fs::create_dir_all(ext_dir.join("core")).unwrap();
    fs::write(ext_dir.join("core/result.ln"), "module std {}").unwrap();
    let manifest = manifest_for(&ext_dir, "result", "core/result", "public");

    let desc = ExternalComponentDiscovery::discover(
        &ext_dir,
        "result",
        &manifest,
        ProviderResolutionContext::UserImport,
    )
    .expect("registered logical provider must resolve");

    assert_eq!(desc.format, ComponentFormat::Source);
    assert_eq!(desc.entry_file, ext_dir.join("core/result.ln"));
}

#[test]
fn llib_candidate_precedes_source_for_registered_provider() {
    let ext_dir = create_temp_dir("llib_over_source");
    fs::create_dir_all(ext_dir.join("alloc")).unwrap();
    fs::write(ext_dir.join("alloc/vec.ln"), "struct Vec {}").unwrap();
    fs::write(ext_dir.join("alloc/vec.llib"), b"candidate").unwrap();
    let manifest = manifest_for(&ext_dir, "vec", "alloc/vec", "public");

    let desc = ExternalComponentDiscovery::discover(
        &ext_dir,
        "vec",
        &manifest,
        ProviderResolutionContext::UserImport,
    )
    .expect("registered provider must produce its artifact candidate");

    assert_eq!(desc.format, ComponentFormat::Llib);
    assert_eq!(desc.entry_file, ext_dir.join("alloc/vec.llib"));
}

#[test]
fn source_is_selected_only_when_artifact_is_absent() {
    let ext_dir = create_temp_dir("source_when_artifact_absent");
    fs::create_dir_all(ext_dir.join("alloc")).unwrap();
    fs::write(ext_dir.join("alloc/string.ln"), "module std {}").unwrap();
    let manifest = manifest_for(&ext_dir, "string", "alloc/string", "public");

    let desc = ExternalComponentDiscovery::discover(
        &ext_dir,
        "string",
        &manifest,
        ProviderResolutionContext::UserImport,
    )
    .expect("source must resolve when no artifact exists");

    assert_eq!(desc.format, ComponentFormat::Source);
    assert_eq!(desc.entry_file, ext_dir.join("alloc/string.ln"));
}

#[test]
fn unregistered_physical_path_is_not_a_logical_alias() {
    let ext_dir = create_temp_dir("physical_path_not_alias");
    fs::create_dir_all(ext_dir.join("alloc")).unwrap();
    fs::write(ext_dir.join("alloc/vec.ln"), "struct Vec {}").unwrap();
    let manifest = manifest_for(&ext_dir, "vec", "alloc/vec", "public");

    let err = ExternalComponentDiscovery::discover(
        &ext_dir,
        "alloc/vec",
        &manifest,
        ProviderResolutionContext::UserImport,
    )
    .expect_err("physical path must not become a logical provider identity");
    assert!(format!("{err:?}").contains("NotFound"));
}

#[test]
fn internal_visibility_depends_on_requester_provenance() {
    let ext_dir = create_temp_dir("internal_provenance");
    fs::create_dir_all(ext_dir.join("alloc")).unwrap();
    fs::write(ext_dir.join("alloc/raw_table.ln"), "struct RawTable {}").unwrap();
    let manifest = manifest_for(&ext_dir, "__raw_table", "alloc/raw_table", "internal");

    assert!(
        ExternalComponentDiscovery::discover(
            &ext_dir,
            "__raw_table",
            &manifest,
            ProviderResolutionContext::UserImport,
        )
        .is_err()
    );
    assert!(
        ExternalComponentDiscovery::discover(
            &ext_dir,
            "__raw_table",
            &manifest,
            ProviderResolutionContext::SysrootDependency,
        )
        .is_ok()
    );
}

/// COMPAT-PRECEDENCE-01: the canonical `.llib` must shadow a legacy `.mlib`.
#[test]
fn llib_candidate_precedes_mlib_for_registered_provider() {
    let ext_dir = create_temp_dir("llib_over_mlib");
    fs::create_dir_all(ext_dir.join("alloc")).unwrap();
    fs::write(ext_dir.join("alloc/vec.mlib"), b"legacy").unwrap();
    fs::write(ext_dir.join("alloc/vec.llib"), b"canonical").unwrap();
    let manifest = manifest_for(&ext_dir, "vec", "alloc/vec", "public");

    let desc = ExternalComponentDiscovery::discover(
        &ext_dir,
        "vec",
        &manifest,
        ProviderResolutionContext::UserImport,
    )
    .expect("registered provider must resolve");

    assert_eq!(desc.format, ComponentFormat::Llib);
    assert_eq!(desc.entry_file, ext_dir.join("alloc/vec.llib"));
}
