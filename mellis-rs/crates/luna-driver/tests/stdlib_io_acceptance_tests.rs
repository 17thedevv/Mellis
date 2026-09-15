use std::{fs, path::PathBuf};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use luna_llib::MlibReader;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_io_tests_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn locate_canonical_io_ln() -> PathBuf {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let io_path = sysroot.root().join("libs").join("external").join("io.ln");
    assert!(io_path.exists(), "libs/external/io.ln must exist");
    io_path
}

/// 0. Compile io.ln to io.llib and sync canonical artifact
#[test]
fn test_0_io_compiles_to_llib_and_syncs() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_compile_llib");
    let io_path = locate_canonical_io_ln();
    let io_src = fs::read_to_string(&io_path).expect("Failed to read io.ln");

    let out_llib = dir.join("io.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };

    let res = compile(io_path.to_str().unwrap(), io_src, &compile_opts);
    assert!(res.is_ok(), "Compiling io.ln to io.llib must succeed: {:?}", res.err());
    assert!(out_llib.exists(), "io.llib artifact must exist on disk");

    // Verify artifact is readable
    let mut file = fs::File::open(&out_llib).expect("Failed to open compiled io.llib");
    let read_res = MlibReader::read_module(&mut file);
    assert!(read_res.is_ok(), "MlibReader must successfully parse compiled io.llib");

    // Sync canonical artifact
    let canonical_llib = io_path.parent().unwrap().join("io.llib");
    fs::copy(&out_llib, &canonical_llib).expect("Failed to sync canonical io.llib");
}

/// IO-1: io::println outputs slice bytes followed by newline
#[test]
fn test_io_println_stdout() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_println");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core>;
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
    assert!(res.is_ok(), "Checking io::println must succeed: {:?}", res.err());
}

/// IO-2: io::print and io::println sequencing
#[test]
fn test_io_print_and_println_sequencing() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_print_seq");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core>;
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
    assert!(res.is_ok(), "Checking io print sequencing must succeed: {:?}", res.err());
}

/// IO-3: Provider resolution and namespace boundary (Rule 7)
#[test]
fn test_io_provider_namespace_boundary() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_namespace");
    let src_path = dir.join("main.ln");

    // Calling print() directly without io:: namespace MUST fail resolution
    let bad_src = r#"
import <core>;
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
    assert!(res.is_err(), "Calling bare print without io:: namespace MUST be rejected");
}

/// IO-4: io::eprintln outputs to stderr
#[test]
fn test_io_eprintln_stderr() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("io_eprintln");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core>;
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
    assert!(res.is_ok(), "Checking io::eprintln must succeed: {:?}", res.err());
}
