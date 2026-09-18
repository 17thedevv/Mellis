use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_reject_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sysroot_root() -> String {
    Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string()
}

const PROVIDER: &str = r#"
module dep {
    export fn answer() -> i32 {
        return 42;
    }
}
"#;

const CONSUMER: &str = r#"
import "dep";

fn main() -> i32 {
    return dep::answer();
}
"#;

fn compile_consumer(dir: &Path, sysroot: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let main_path = dir.join("main.ln");
    fs::write(&main_path, CONSUMER).unwrap();
    let opts = CompilerOptions {
        output_path: Some(dir.join("main.exe").to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: false,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    compile(main_path.to_str().unwrap(), CONSUMER.to_string(), &opts)
}

#[test]
fn test_missing_llib_source_compiles() {
    let dir = temp_dir("missing");
    let sysroot = sysroot_root();

    fs::write(dir.join("dep.ln"), PROVIDER).unwrap();

    let res = compile_consumer(&dir, &sysroot);
    assert!(
        res.is_ok(),
        "Source-only provider must be discoverable when .llib is absent: {:?}",
        res.err()
    );
}

#[test]
fn test_existing_invalid_llib_rejected_without_source_fallback() {
    let dir = temp_dir("invalid");
    let sysroot = sysroot_root();

    // Valid source is present, so a fallback-to-source implementation would succeed here.
    fs::write(dir.join("dep.ln"), PROVIDER).unwrap();
    // Corrupt .llib with an invalid magic.
    let _ = fs::write(dir.join("dep.llib"), b"this is not a valid artifact");

    let res = compile_consumer(&dir, &sysroot);
    assert!(
        res.is_err(),
        "Existing invalid .llib must be rejected, not silently fall back to source"
    );
}

#[test]
fn test_truncated_llib_rejected() {
    let dir = temp_dir("truncated");
    let sysroot = sysroot_root();

    // Build a valid dep.llib first.
    let dep_ln = dir.join("dep.ln");
    fs::write(&dep_ln, PROVIDER).unwrap();
    let dep_llib = dir.join("dep.llib");
    let build_opts = CompilerOptions {
        output_path: Some(dep_llib.to_string_lossy().to_string()),
        search_paths: vec![sysroot.clone()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let build_res = compile(dep_ln.to_str().unwrap(), PROVIDER.to_string(), &build_opts);
    assert!(
        build_res.is_ok(),
        "Building valid dep.llib failed: {:?}",
        build_res.err()
    );
    assert!(dep_llib.exists());

    // Truncate well below the 122-byte header.
    let bytes = fs::read(&dep_llib).unwrap();
    let _ = fs::write(&dep_llib, &bytes[..60.min(bytes.len())]);

    let res = compile_consumer(&dir, &sysroot);
    assert!(
        res.is_err(),
        "Truncated .llib must be rejected deterministically"
    );
}

/// Rewrite the `section_type` of the AstInterface section (id 15) to Custom
/// (0xFFFFFFFF). The artifact keeps a valid header, Manifest, and
/// SemanticMetadata, but has no recognizable AstInterface section.
fn strip_ast_interface(llb: &Path) {
    let mut bytes = fs::read(llb).unwrap();
    let section_count = u32::from_le_bytes(bytes[110..114].try_into().unwrap()) as usize;
    let table_off = u64::from_le_bytes(bytes[114..122].try_into().unwrap()) as usize;
    for i in 0..section_count {
        let e = table_off + i * 40;
        let stype = u32::from_le_bytes(bytes[e + 4..e + 8].try_into().unwrap());
        if stype == 15 {
            bytes[e + 4..e + 8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        }
    }
    fs::write(llb, bytes).unwrap();
}

fn build_dep_llib(dir: &Path, sysroot: &str) -> PathBuf {
    let dep_ln = dir.join("dep.ln");
    let dep_llib = dir.join("dep.llib");
    fs::write(&dep_ln, PROVIDER).unwrap();
    let opts = CompilerOptions {
        output_path: Some(dep_llib.to_string_lossy().to_string()),
        search_paths: vec![sysroot.to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(dep_ln.to_str().unwrap(), PROVIDER.to_string(), &opts);
    assert!(res.is_ok(), "build dep.llib failed: {:?}", res.err());
    assert!(dep_llib.exists());
    dep_llib
}

/// ARTIFACT-PARITY-01 classification (A): a canonical `.llib` MUST carry the
/// AstInterface section (required semantic authority). A `.llib` that lacks it
/// and only offers SemanticMetadata is rejected, NOT silently accepted as a
/// semantically-lossy fallback.
#[test]
fn test_canonical_llib_missing_ast_interface_rejected() {
    let dir = temp_dir("no_astiface_llib");
    let sysroot = sysroot_root();

    let dep_llib = build_dep_llib(&dir, &sysroot);
    strip_ast_interface(&dep_llib);

    let res = compile_consumer(&dir, &sysroot);
    assert!(
        res.is_err(),
        "canonical .llib missing AstInterface must be rejected"
    );
}

/// The SemanticMetadata-only path remains available for legacy `.mlib`
/// compatibility artifacts, and must NOT be rejected by the canonical `.llib`
/// AstInterface rule.
#[test]
fn test_legacy_mlib_without_ast_interface_not_rejected_as_canonical_llib() {
    let dir = temp_dir("no_astiface_mlib");
    let sysroot = sysroot_root();

    let dep_llib = build_dep_llib(&dir, &sysroot);
    strip_ast_interface(&dep_llib);
    let dep_mlib = dir.join("dep.mlib");
    fs::copy(&dep_llib, &dep_mlib).unwrap();
    fs::remove_file(&dep_llib).unwrap();

    let res = compile_consumer(&dir, &sysroot);
    let err_str = format!("{:?}", res.err());
    assert!(
        !err_str.contains("missing the required AstInterface"),
        "legacy .mlib must not be rejected by the canonical .llib AstInterface rule; got: {}",
        err_str
    );
}
