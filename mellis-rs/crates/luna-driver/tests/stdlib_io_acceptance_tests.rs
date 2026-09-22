use luna_driver::sysroot::Sysroot;
use luna_driver::sysroot_builder::SysrootBuilder;
use luna_driver::{compile, CompilerOptions};
use luna_llib::MlibReader;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_io_tests_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn locate_canonical_io_ln() -> PathBuf {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let io_path = sysroot
        .root()
        .join("libs")
        .join("external")
        .join("io")
        .join("io.ln");
    assert!(io_path.exists(), "libs/external/io/io.ln must exist");
    io_path
}

fn locate_runtime_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("luna")
        .join("stdlib")
        .join("io")
        .join("io_runtime.ln")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("Failed to create temporary sysroot directory");
    for entry in fs::read_dir(source).expect("Failed to enumerate canonical sysroot") {
        let entry = entry.expect("Failed to read canonical sysroot entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if let Some(name) = source_path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else if source_path.exists() {
            let _ = fs::copy(&source_path, &destination_path);
        }
    }
}

fn compile_and_run_fixture(
    fixture: &Path,
    source: &str,
    sysroot_root: &Path,
    exe_path: &Path,
) -> (i32, String, String) {
    let opts = CompilerOptions {
        output_path: Some(exe_path.to_string_lossy().to_string()),
        search_paths: vec![sysroot_root.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let result = compile(fixture.to_str().unwrap(), source.to_string(), &opts);
    assert!(
        result.is_ok(),
        "IO runtime fixture must compile: {:?}",
        result.err()
    );

    let output = Command::new(exe_path)
        .output()
        .expect("IO runtime fixture must execute");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n"),
    )
}

/// IO-0: The canonical SysrootBuilder owns canonical artifact generation.
#[test]
fn test_0_sysroot_builder_rebuilds_nested_io_artifacts() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let io_path = locate_canonical_io_ln();
    let external_dir = sysroot.external_dir().to_path_buf();
    let builder = SysrootBuilder::new(sysroot);
    builder
        .build_all(true)
        .expect("SysrootBuilder must rebuild the canonical sysroot");

    let canonical_llib = io_path.with_extension("llib");
    let canonical_obj = io_path.with_extension("obj");
    assert!(
        canonical_llib.exists(),
        "io/io.llib must exist after the canonical rebuild"
    );
    assert!(
        canonical_obj.exists(),
        "io/io.obj must exist when the canonical builder emits an object sidecar"
    );
    assert!(
        !external_dir.join("io.llib").exists(),
        "obsolete root io.llib must be removed"
    );
    assert!(
        !external_dir.join("io.obj").exists(),
        "obsolete root io.obj must be removed"
    );

    // Verify artifact is readable and records the dependency closure proven by 4C.
    let mut file = fs::File::open(&canonical_llib).expect("Failed to open canonical io/io.llib");
    let (_, manifest, _, _) = MlibReader::read_module(&mut file)
        .expect("MlibReader must successfully parse canonical io/io.llib");
    let manifest = manifest.expect("canonical io/io.llib must contain a manifest");
    assert!(
        !manifest.dependencies.deps.is_empty(),
        "IO now records stdlib dependencies (result, string, vec, etc.)"
    );
}

/// IO-1: io::println outputs slice bytes followed by newline
#[test]
fn test_io_println_stdout() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_println");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;
import <io>;

fn main() -> i32 {
    dec msg: [u8; 5] = [72 as u8, 101 as u8, 108 as u8, 108 as u8, 111 as u8];
    io::println(&msg);
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = luna_driver::check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Checking io::println must succeed: {:?}",
        res.err()
    );
}

/// IO-2: io::print and io::println sequencing
#[test]
fn test_io_print_and_println_sequencing() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_print_seq");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;
import <io>;

fn main() -> i32 {
    dec part1: [u8; 7] = [72 as u8, 101 as u8, 108 as u8, 108 as u8, 111 as u8, 44 as u8, 32 as u8];
    dec part2: [u8; 6] = [87 as u8, 111 as u8, 114 as u8, 108 as u8, 100 as u8, 33 as u8];
    io::print(&part1);
    io::println(&part2);
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = luna_driver::check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Checking io print sequencing must succeed: {:?}",
        res.err()
    );
}

/// IO-3: Provider resolution and namespace boundary (Rule 7)
#[test]
fn test_io_provider_namespace_boundary() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_namespace");
    let src_path = dir.join("main.ln");

    // Calling print() directly without io:: namespace MUST fail resolution
    let bad_src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;
import <io>;

fn main() -> i32 {
    dec msg: [u8; 4] = [84 as u8, 101 as u8, 115 as u8, 116 as u8];
    print(&msg);
    return 0;
}
"#;
    fs::write(&src_path, bad_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = luna_driver::check(src_path.to_str().unwrap(), bad_src.to_string(), &opts);
    assert!(
        res.is_err(),
        "Calling bare print without io:: namespace MUST be rejected"
    );
}

/// IO-4: io::eprintln outputs to stderr
#[test]
fn test_io_eprintln_stderr() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_eprintln");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;
import <io>;

fn main() -> i32 {
    dec err_msg: [u8; 6] = [69 as u8, 114 as u8, 114 as u8, 111 as u8, 114 as u8, 33 as u8];
    io::eprintln(&err_msg);
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = luna_driver::check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Checking io::eprintln must succeed: {:?}",
        res.err()
    );
}

/// IO-5: Physical layout is not a logical provider ID.
#[test]
fn test_io_physical_path_is_not_importable() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_physical_path_rejected");
    let src_path = dir.join("main.ln");
    let src = "import <io/io>; fn main() -> i32 { return 0; }";
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = luna_driver::check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "import <io/io> must not resolve as a logical provider"
    );
}

/// IO-6: Standalone Luna fixture has observable source/.llib parity.
#[test]
fn test_io_runtime_fixture_source_and_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let fixture = locate_runtime_fixture();
    let source = fs::read_to_string(&fixture).expect("Failed to read IO runtime fixture");
    let dir = create_temp_dir("io_runtime_fixture");
    let artifact_exe = dir.join(if cfg!(windows) {
        "io_artifact.exe"
    } else {
        "io_artifact"
    });
    let source_exe = dir.join(if cfg!(windows) {
        "io_source.exe"
    } else {
        "io_source"
    });

    let artifact_result = compile_and_run_fixture(&fixture, &source, sysroot.root(), &artifact_exe);

    let source_sysroot = dir.join("source_sysroot");
    let source_external = source_sysroot.join("libs").join("external");
    copy_tree(sysroot.external_dir(), &source_external);
    fs::remove_file(source_external.join("io").join("io.llib"))
        .expect("Temporary source-backed sysroot must remove io/io.llib");
    let _ = fs::remove_file(source_external.join("io").join("io.obj"));

    let source_result = compile_and_run_fixture(&fixture, &source, &source_sysroot, &source_exe);

    assert_eq!(artifact_result, source_result);
    assert_eq!(artifact_result.0, 0);
    assert_eq!(artifact_result.1, "Hello, World!\n");
    assert_eq!(artifact_result.2, "Error!\n");
}
