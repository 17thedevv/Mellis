use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_binary(exe_path: &Path) -> (i32, String, String) {
    let output = Command::new(exe_path)
        .output()
        .expect("Failed to execute produced binary");
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (exit_code, stdout, stderr)
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_slice_index_tests_{}", name));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 1. Positive slice index read
#[test]
fn test_slice_index_read() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_read");
    let _opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn test_read() -> bool {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    v.push(30);

    dec s = v.as_slice();
    dec x0 = s[0];
    dec x1 = s[1];
    dec x2 = s[2];

    return (x0 == 10) && (x1 == 20) && (x2 == 30);
}

fn main() -> i32 {
    if test_read() {
        return 0;
    } else {
        return 1;
    }
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let bin_path = dir.join("main.exe");
    let compile_opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = compile(path.to_str().unwrap(), src.to_string(), &compile_opts);
    assert!(res.is_ok(), "Compiling slice index read must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Binary must exit with code 0 (all reads correct)");
}

/// 2. Positive slice index write
#[test]
fn test_slice_index_write() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_write");

    let src = r#"
import <core>;
import <alloc>;

fn test_write() -> bool {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    v.push(30);

    dec rw s = v.as_mut_slice();
    s[1] = 99;

    dec s_read = v.as_slice();
    return (s_read[0] == 10) && (s_read[1] == 99) && (s_read[2] == 30);
}

fn main() -> i32 {
    if test_write() {
        return 0;
    } else {
        return 1;
    }
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let bin_path = dir.join("main.exe");
    let compile_opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = compile(path.to_str().unwrap(), src.to_string(), &compile_opts);
    assert!(res.is_ok(), "Compiling slice index write must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Binary must exit with code 0 (mutation reflected)");
}

/// 3. Borrowck rejects mutating Vec while slice element reference is alive
#[test]
fn test_slice_index_borrow_conflict() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_borrow_conflict");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn bad_mutation() {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);

    dec s = v.as_slice();
    dec elem_ref = &s[0];
    v.push(30); // conflict: mutating vector while slice element reference is alive
    dec val = *elem_ref;
}

fn main() {
    bad_mutation();
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = compile(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Mutating Vec while slice element reference is live MUST reject!");
}

/// 4. Borrowck allows mutation after element reference is dropped
#[test]
fn test_slice_index_borrow_after_drop_allowed() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_borrow_after_drop");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn allowed_seq() {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);

    {
        dec s = v.as_slice();
        dec elem_val = s[0];
    }

    v.push(30); // permitted because slice is dead
}

fn main() {
    allowed_seq();
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = compile(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Non-overlapping slice indexing must pass borrowck: {:?}", res.err());
}

/// 5. Out-of-bounds slice index panics at runtime
#[test]
fn test_slice_index_out_of_bounds_aborts() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_oob");

    let src = r#"
import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);

    dec s = v.as_slice();
    dec x = s[5]; // Out of bounds: len is 2, index is 5

    return x;
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let bin_path = dir.join("main.exe");
    let compile_opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = compile(path.to_str().unwrap(), src.to_string(), &compile_opts);
    assert!(res.is_ok(), "Compiling out of bounds test must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_ne!(exit_code, 0, "Out-of-bounds access MUST fail/abort at runtime");
}

/// 6. Source .ln vs .llib parity for slice indexing
#[test]
fn test_slice_index_source_and_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("index_parity");

    let lib_src_path = dir.join("indexer.ln");
    let lib_bin_path = dir.join("indexer.llib");
    let consumer_path = dir.join("consumer.ln");

    let lib_src = r#"
import <core>;
import <alloc>;

module indexer {
    export fn sum_first_two(s: &[i32]) -> i32 {
        return s[0] + s[1];
    }
}
"#;
    fs::write(&lib_src_path, lib_src).unwrap();

    let compile_opts = CompilerOptions {
        output_path: Some(lib_bin_path.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };
    let res_compile = compile(lib_src_path.to_str().unwrap(), lib_src.to_string(), &compile_opts);
    assert!(res_compile.is_ok(), "Compiling indexer.ln to .llib MUST succeed: {:?}", res_compile.err());

    let consumer_src = r#"
import <core>;
import <alloc>;
import "indexer";

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(32);
    dec total = indexer::sum_first_two(v.as_slice());
    if total == 42 {
        return 0;
    } else {
        return 1;
    }
}
"#;
    fs::write(&consumer_path, consumer_src).unwrap();

    let consumer_opts = CompilerOptions {
        no_link: true,
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        quiet: true,
        ..Default::default()
    };

    let res_consumer = compile(consumer_path.to_str().unwrap(), consumer_src.to_string(), &consumer_opts);
    assert!(res_consumer.is_ok(), "Consumer using .llib slice indexing MUST compile: {:?}", res_consumer.err());
}
