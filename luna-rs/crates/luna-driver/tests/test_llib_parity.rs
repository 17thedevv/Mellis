//! Canonical source/.llib parity (renamed from the stale `test_mlib_parity`).
//!
//! Compiles a provider to a canonical `.llib` and verifies a consumer produces the
//! same observable result whether the provider is resolved from source (`.ln`) or
//! from the canonical `.llib` artifact. The legacy `Mlib`-mode branch that was
//! commented out previously is intentionally dropped: canonical parity is proven
//! through `.llib`.
use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_llib_parity_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

const PROVIDER: &str = r#"
module prov {
    export struct Container<T> {
        export value: T,
    };

    export fn create_container(val: i32) -> Container<i32> {
        return Container<i32> { value: val };
    }

    export fn do_math(a: i32, b: i32) -> i32 {
        return a + b;
    }
}
"#;

const CONSUMER: &str = r#"
import "prov";

fn main() -> i32 {
    dec c = prov::create_container(10);
    dec m = prov::do_math(20, 20);
    if c.value + m == 50 {
        return 0;
    }
    return 1;
}
"#;

fn run_consumer(dir: &Path, sysroot: &str) -> (bool, i32) {
    let main_path = dir.join("main.ln");
    let exe = dir.join("main.exe");
    let _ = fs::remove_file(&exe);
    fs::write(&main_path, CONSUMER).unwrap();
    let opts = CompilerOptions {
        output_path: Some(exe.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    if compile(main_path.to_str().unwrap(), CONSUMER.to_string(), &opts).is_err() {
        return (false, -1);
    }
    let out = Command::new(&exe).output().expect("run consumer");
    (true, out.status.code().unwrap_or(-2))
}

#[test]
fn test_source_and_canonical_llib_parity() {
    let sysroot = Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string();

    // Source mode: provider resolved from `.ln`.
    let dir_src = temp_dir("source");
    fs::write(dir_src.join("prov.ln"), PROVIDER).unwrap();
    let (ok_src, code_src) = run_consumer(&dir_src, &sysroot);
    assert!(ok_src, "source-mode consumer must compile against prov.ln");
    assert_eq!(code_src, 0, "source-mode observable result must be 0");

    // Canonical `.llib` mode: provider resolved from `prov.llib` only.
    let dir_lib = temp_dir("llib");
    let prov_ln = dir_lib.join("prov.ln");
    let prov_llib = dir_lib.join("prov.llib");
    fs::write(&prov_ln, PROVIDER).unwrap();
    let opts = CompilerOptions {
        output_path: Some(prov_llib.to_string_lossy().to_string()),
        search_paths: vec![dir_lib.to_string_lossy().to_string(), sysroot.clone()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(prov_ln.to_str().unwrap(), PROVIDER.to_string(), &opts);
    assert!(res.is_ok(), "building prov.llib failed: {:?}", res.err());
    assert!(prov_llib.exists(), "prov.llib must exist");
    fs::remove_file(&prov_ln).unwrap();

    let (ok_lib, code_lib) = run_consumer(&dir_lib, &sysroot);
    assert!(ok_lib, "`.llib`-mode consumer must compile against prov.llib");
    assert_eq!(code_lib, 0, "`.llib`-mode observable result must be 0");

    assert_eq!(
        code_src, code_lib,
        "source and canonical .llib must be observationally equivalent"
    );
}

/// COMPAT-READ-01: legacy `.mlib` read compatibility. A `.mlib` produced by the
/// current writer (it carries `AstInterface`) must still load and behave correctly
/// when it is the only representation available.
#[test]
fn test_legacy_mlib_read_compatibility_end_to_end() {
    let sysroot = Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string();

    let dir = temp_dir("legacy_mlib");
    let prov_ln = dir.join("prov.ln");
    let prov_mlib = dir.join("prov.mlib");
    fs::write(&prov_ln, PROVIDER).unwrap();
    let opts = CompilerOptions {
        output_path: Some(prov_mlib.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.clone()],
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(prov_ln.to_str().unwrap(), PROVIDER.to_string(), &opts);
    assert!(res.is_ok(), "building prov.mlib failed: {:?}", res.err());
    assert!(prov_mlib.exists(), "prov.mlib must exist");
    fs::remove_file(&prov_ln).unwrap();

    let (ok, code) = run_consumer(&dir, &sysroot);
    assert!(ok, "`.mlib`-mode consumer must compile against prov.mlib");
    assert_eq!(code, 0, "legacy .mlib read must be observationally correct");
}
