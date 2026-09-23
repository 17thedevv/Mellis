use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_freshness_{}_{}",
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

fn build_provider(dir: &Path, name: &str, src: &str, sysroot: &str) {
    let ln = dir.join(format!("{}.ln", name));
    let llib = dir.join(format!("{}.llib", name));
    fs::write(&ln, src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(llib.to_string_lossy().to_string()),
        search_paths: vec![sysroot.to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(ln.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "build provider {} failed: {:?}",
        name,
        res.err()
    );
}

fn compile_consumer(dir: &Path, src: &str, sysroot: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let main_path = dir.join("main.ln");
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(dir.join("main.exe").to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: false,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    compile(main_path.to_str().unwrap(), src.to_string(), &opts)
}

const A_V1: &str = r#"
module a {
    export fn value() -> i32 {
        return 1;
    }
}
"#;

const A_V2_INTERFACE: &str = r#"
module a {
    export fn value() -> i32 {
        return 1;
    }

    export fn extra() -> i32 {
        return 2;
    }
}
"#;

const A_V2_IMPL_ONLY: &str = r#"
module a {
    export fn value() -> i32 {
        return 2;
    }
}
"#;

const B: &str = r#"
import "a";

module b {
    export fn use_value() -> i32 {
        return a::value();
    }
}
"#;

const C_IMPORT_B: &str = r#"
import "b";

fn main() -> i32 {
    return b::use_value();
}
"#;

const C_IMPORT_A: &str = r#"
import "a";

fn main() -> i32 {
    return a::value();
}
"#;

#[test]
fn test_stale_source_fingerprint_rejected() {
    let dir = temp_dir("self_stale");
    let sysroot = sysroot_root();

    build_provider(&dir, "a", A_V1, &sysroot);

    // Mutate source WITHOUT rebuilding the .llib.
    fs::write(dir.join("a.ln"), A_V2_IMPL_ONLY).unwrap();

    let res = compile_consumer(&dir, C_IMPORT_A, &sysroot);
    assert!(
        res.is_err(),
        "Stale source (changed .ln, unchanged .llib) must be rejected, not silently recompiled: {:?}",
        res.err()
    );
}

#[test]
fn test_dependency_interface_change_invalidates_consumer() {
    let dir = temp_dir("dep_interface_change");
    let sysroot = sysroot_root();

    build_provider(&dir, "a", A_V1, &sysroot);
    build_provider(&dir, "b", B, &sysroot);

    // Rebuild A with a changed public interface (added export) without rebuilding B.
    build_provider(&dir, "a", A_V2_INTERFACE, &sysroot);

    let res = compile_consumer(&dir, C_IMPORT_B, &sysroot);
    assert!(
        res.is_err(),
        "Stale dependency (A interface changed, B not rebuilt) must invalidate B: {:?}",
        res.err()
    );
}

#[test]
fn test_dependency_implementation_only_change_preserves_validity() {
    let dir = temp_dir("dep_impl_only");
    let sysroot = sysroot_root();

    build_provider(&dir, "a", A_V1, &sysroot);
    build_provider(&dir, "b", B, &sysroot);

    // Rebuild A with only a body change (identical public interface).
    build_provider(&dir, "a", A_V2_IMPL_ONLY, &sysroot);

    let res = compile_consumer(&dir, C_IMPORT_B, &sysroot);
    assert!(
        res.is_ok(),
        "Implementation-only change must NOT invalidate dependent B: {:?}",
        res.err()
    );
}
