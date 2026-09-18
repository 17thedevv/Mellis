//! Phase 4G: legacy compatibility regressions.
//!
//! - COMPAT-WRITE-01: a canonical build emits `.llib` only (never a legacy `.mlib`).
//! - COMPAT-PRECEDENCE-01: canonical source (`.ln`) precedes a legacy `.mlib` artifact.
//! - COMPAT-READ-01: legacy `.ms` source is still read through the resolver.
use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_legacy_compat_{}_{}",
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

/// Compile `src` from a scratch source file into `out`, with explicit emission flags.
fn compile_src(dir: &Path, src: &str, out: &Path, sysroot: &str, emit_llib: bool, emit_mlib: bool) {
    let src_path = dir.join("tmp_src.ln");
    fs::write(&src_path, src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(out.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib,
        emit_mlib,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "compile failed: {:?}", res.err());
}

/// Compile + link + run a `main` source in `dir`, returning its exit code.
fn run_main(dir: &Path, src: &str, sysroot: &str) -> i32 {
    let main_path = dir.join("main.ln");
    let exe = dir.join("main.exe");
    let _ = fs::remove_file(&exe);
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(exe.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "consumer compile failed: {:?}", res.err());
    Command::new(&exe)
        .output()
        .expect("run consumer")
        .status
        .code()
        .unwrap_or(-99)
}

const PROV_ONE: &str = r#"
module foo {
    export fn val() -> i32 { return 1; }
}
"#;

const PROV_TWO: &str = r#"
module foo {
    export fn val() -> i32 { return 2; }
}
"#;

const MAIN_CALL_VAL: &str = r#"
import "foo";

fn main() -> i32 {
    return foo::val();
}
"#;

/// COMPAT-WRITE-01: a canonical build emits `.llib` and must NOT also emit `.mlib`.
#[test]
fn test_canonical_build_emits_llib_only_not_mlib() {
    let dir = temp_dir("emit_canonical");
    let sysroot = sysroot_root();
    let llib = dir.join("foo.llib");
    let mlib = dir.join("foo.mlib");

    compile_src(&dir, PROV_ONE, &llib, &sysroot, true, false);

    assert!(llib.exists(), "canonical build must emit .llib");
    assert!(
        !mlib.exists(),
        "canonical build must NOT emit a legacy .mlib"
    );
}

/// COMPAT-PRECEDENCE-01: canonical source (`.ln`) precedes a legacy `.mlib` artifact.
#[test]
fn test_local_source_precedes_legacy_mlib_artifact() {
    let dir = temp_dir("ln_over_mlib");
    let sysroot = sysroot_root();

    // Legacy artifact named `foo.mlib` whose `val` returns 2.
    let mlib = dir.join("foo.mlib");
    compile_src(&dir, PROV_TWO, &mlib, &sysroot, false, true);
    assert!(mlib.exists(), "legacy .mlib must be emitted");
    let _ = fs::remove_file(dir.join("tmp_src.ln"));

    // Canonical source named `foo.ln` whose `val` returns 1.
    fs::write(dir.join("foo.ln"), PROV_ONE).unwrap();

    let code = run_main(&dir, MAIN_CALL_VAL, &sysroot);
    assert_eq!(
        code, 1,
        "canonical .ln must precede legacy .mlib (got exit code {})",
        code
    );
}

/// COMPAT-READ-01: a legacy `.ms` provider is still read through the resolver.
#[test]
fn test_legacy_ms_source_read_end_to_end() {
    let dir = temp_dir("legacy_ms");
    let sysroot = sysroot_root();

    // Only a legacy `.ms` representation exists (no `.llib`/`.mlib`/`.ln`).
    fs::write(dir.join("foo.ms"), PROV_ONE.replace("return 1", "return 7")).unwrap();

    let code = run_main(&dir, MAIN_CALL_VAL, &sysroot);
    assert_eq!(
        code, 7,
        "legacy .ms source must be read via the resolver (got exit code {})",
        code
    );
}
