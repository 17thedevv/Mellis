use std::fs;
use std::path::PathBuf;
use luna_driver::discovery::{ExternalComponentDiscovery, ComponentFormat};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("luna_precedence_tests").join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

#[test]
fn test_canonical_ln_over_legacy_ms() {
    let ext_dir = create_temp_dir("ln_over_ms");

    fs::write(ext_dir.join("foo.ln"), "module foo {}").unwrap();
    fs::write(ext_dir.join("foo.ms"), "module foo_legacy {}").unwrap();

    let desc = ExternalComponentDiscovery::discover(&ext_dir, "foo").expect("should discover foo");
    assert_eq!(desc.format, ComponentFormat::Source);
    assert_eq!(desc.entry_file, ext_dir.join("foo.ln"));
}

#[test]
fn test_canonical_llib_over_legacy_mlib() {
    let ext_dir = create_temp_dir("llib_over_mlib");

    fs::write(ext_dir.join("bar.llib"), b"LLIB").unwrap();
    fs::write(ext_dir.join("bar.mlib"), b"MLIB").unwrap();

    let desc = ExternalComponentDiscovery::discover(&ext_dir, "bar").expect("should discover bar");
    assert_eq!(desc.format, ComponentFormat::Llib);
    assert_eq!(desc.entry_file, ext_dir.join("bar.llib"));
}

#[test]
fn test_canonical_source_over_legacy_mlib() {
    let ext_dir = create_temp_dir("source_over_mlib");

    // Canonical source .ln must win over legacy compiled .mlib (no legacy shadowing)
    fs::write(ext_dir.join("baz.ln"), "module baz {}").unwrap();
    fs::write(ext_dir.join("baz.mlib"), b"MLIB").unwrap();

    let desc = ExternalComponentDiscovery::discover(&ext_dir, "baz").expect("should discover baz");
    assert_eq!(desc.format, ComponentFormat::Source);
    assert_eq!(desc.entry_file, ext_dir.join("baz.ln"));
}

#[test]
fn test_legacy_ms_fallback_when_canonical_absent() {
    let ext_dir = create_temp_dir("legacy_ms_fallback");

    fs::write(ext_dir.join("legacy_mod.ms"), "module legacy_mod {}").unwrap();

    let desc = ExternalComponentDiscovery::discover(&ext_dir, "legacy_mod").expect("should discover legacy_mod");
    assert_eq!(desc.format, ComponentFormat::Source);
    assert_eq!(desc.entry_file, ext_dir.join("legacy_mod.ms"));
}

#[test]
fn test_legacy_mlib_fallback_when_canonical_absent() {
    let ext_dir = create_temp_dir("legacy_mlib_fallback");

    fs::write(ext_dir.join("legacy_lib.mlib"), b"MLIB").unwrap();

    let desc = ExternalComponentDiscovery::discover(&ext_dir, "legacy_lib").expect("should discover legacy_lib");
    assert_eq!(desc.format, ComponentFormat::Llib);
    assert_eq!(desc.entry_file, ext_dir.join("legacy_lib.mlib"));
}
