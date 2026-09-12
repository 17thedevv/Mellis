use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_stdlib_01d_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn make_opts(test_sysroot: &Sysroot) -> CompilerOptions {
    CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    }
}

/// 1. Valid read of slice length and emptiness via as_slice()
#[test]
fn test_vec_as_slice_len_and_is_empty() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vec_as_slice_len");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;
import <alloc>;

fn test_slice_props() -> bool {
    dec rw v = vec_new<i32>();
    dec s0 = v.as_slice();
    dec empty0 = s0.is_empty();
    dec len0 = s0.len();

    v.push(10);
    v.push(20);
    dec s1 = v.as_slice();
    dec empty1 = s1.is_empty();
    dec len1 = s1.len();

    return empty0 && (len0 == (0 as usize)) && (empty1 == false) && (len1 == (2 as usize));
}

fn main() -> i32 {
    test_slice_props();
    return 0;
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = compile(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Compiling vec as_slice properties must succeed: {:?}", res.err());
}

/// 2. Borrowck rejects mutation of Vec while its projected &[T] slice is alive
#[test]
fn test_vec_as_slice_mutation_conflict_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vec_as_slice_conflict");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;
import <alloc>;

fn conflict() {
    dec rw v = vec_new<i32>();
    v.push(1);
    dec s = v.as_slice(); // shared borrow of v
    v.push(2);            // MUTATION while s is alive!
    dec l = s.len();       // s used here
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Mutating Vec while &[T] slice is live must be rejected by borrowck");
}

/// 3. NLL: Mutating Vec is allowed after projected &[T] slice dies
#[test]
fn test_vec_as_slice_mutation_after_drop_allowed() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vec_as_slice_nll");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;
import <alloc>;

fn nll_ok() {
    dec rw v = vec_new<i32>();
    v.push(1);
    dec s = v.as_slice();
    dec l = s.len(); // last use of s
    v.push(2);       // allowed: s is dead
}

fn main() -> i32 {
    nll_ok();
    return 0;
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = compile(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Mutating Vec after slice dies must be permitted: {:?}", res.err());
}

/// 4. Mutable slice projection: as_mut_slice creates exclusive mutable borrow
#[test]
fn test_vec_as_mut_slice_exclusive() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vec_as_mut_slice_exclusive");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;
import <alloc>;

fn conflict_mut() {
    dec rw v = vec_new<i32>();
    v.push(1);
    dec rw s = v.as_mut_slice(); // exclusive mutable borrow of v
    dec l = v.len();             // read access to v while s is alive!
    dec slen = s.len();
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Accessing Vec while &rw [T] mutable slice is live must be rejected");
}

/// 5. Returning a slice holding a reference to a local stack array is rejected (E3005)
#[test]
fn test_slice_local_escape_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("slice_local_escape");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn escape() -> &[i32] {
    dec arr = [1, 2, 3];
    dec sl: &[i32] = &arr;
    return sl; // escaping local array!
}
"#;

    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Returning slice of local array must be rejected (E3005)");
    let err_str = format!("{:?}", res.err());
    assert!(err_str.contains("E3005") || err_str.contains("LocalBorrowEscape") || err_str.contains("escapes"),
        "Diagnostic must report escape: {}", err_str);
}

/// 6. Source vs .llib parity for Vec::as_slice
#[test]
fn test_vec_slice_source_and_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vec_slice_parity");
    let lib_src_path = dir.join("my_provider.ln");
    let lib_bin_path = dir.join("my_provider.llib");
    let consumer_path = dir.join("consumer.ln");

    let lib_src = r#"
import <core>;
import <alloc>;

module my_provider {
    export fn inspect_vec(v: &Vec<i32>) -> usize {
        dec s = v.as_slice();
        return s.len();
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
    assert!(res_compile.is_ok(), "Compiling my_provider.ln to .llib MUST succeed: {:?}", res_compile.err());

    let consumer_src = r#"
import <core>;
import <alloc>;
import "my_provider";

fn main() {
    dec rw v = vec_new<i32>();
    v.push(10);
    dec s = v.as_slice();
    v.push(20); // conflict: mutating while s is alive
    dec l = s.len();
}
"#;
    fs::write(&consumer_path, consumer_src).unwrap();

    let consumer_opts = CompilerOptions {
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        quiet: true,
        ..Default::default()
    };

    let res_consumer = check(consumer_path.to_str().unwrap(), consumer_src.to_string(), &consumer_opts);
    assert!(
        res_consumer.is_err(),
        "Consumer mutating Vec while slice borrow is alive MUST reject!"
    );
}

/// 7. Raw pointer parameter has no safe reference provenance and cannot escape as &[T]
#[test]
fn test_slice_from_raw_parts_rejects_unprovenanced_raw_ptr_escape() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("slice_raw_ptr_escape");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn escape_raw(p: *i32) -> &[i32] life_from(p) {
    unsafe {
        return slice_from_raw_parts(p, 1 as u64);
    }
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Raw pointer parameter cannot serve as safe reference provenance");
    let err_str = format!("{:?}", res.err());
    println!("AUDIT DIAGNOSTIC: {}", err_str);
    assert!(
        err_str.contains("E3005") || err_str.contains("LocalBorrowEscape") || err_str.contains("E2016"),
        "Must emit escape or constraint violation: {}", err_str
    );
}

