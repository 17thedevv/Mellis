use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_stdlib_01a_acceptance_tests")
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

/// 1. Option::as_ref valid read
#[test]
fn test_opt_as_ref_valid_read() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_as_ref_valid_read");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn read_opt(opt: &Option<i32>) -> i32 {
    dec ref_opt = opt.as_ref();
    if ref_opt.is_some() {
        match ref_opt {
            Option::Some(r) -> { return *r; },
            Option::None -> { return 0; },
        }
    }
    return 0;
}

fn main() -> i32 {
    dec o = Option::Some(42);
    return read_opt(&o);
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Option::as_ref valid read MUST succeed: {:?}", res.err());
}

/// 2. Option::as_ref mutation conflict rejected
#[test]
fn test_opt_as_ref_mutation_conflict_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_as_ref_mutation_conflict");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() {
    dec rw opt = Option::Some(10);
    dec r = opt.as_ref();
    opt = Option::Some(20); // Mutating owner while `r` is live MUST be rejected!
    match r {
        Option::Some(val) -> { dec _v = *val; },
        Option::None -> (),
    }
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Mutating Option while borrowed via as_ref() MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("borrowed") || d.message.contains("Cannot write") || d.message.contains("conflict")),
        "Expected borrow conflict error, got: {:?}",
        errs
    );
}

/// 3. Option::as_ref mutation allowed after borrow dies (NLL)
#[test]
fn test_opt_as_ref_mutation_after_drop_allowed() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_as_ref_after_drop");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() {
    dec rw opt = Option::Some(10);
    dec r = opt.as_ref();
    match r {
        Option::Some(val) -> { dec _v1 = *val; },
        Option::None -> (),
    }
    // `r` is dead now; mutating `opt` MUST succeed
    opt = Option::Some(30);
    dec r2 = opt.as_ref();
    match r2 {
        Option::Some(val2) -> { dec _v2 = *val2; },
        Option::None -> (),
    }
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Mutating Option after as_ref borrow dies MUST succeed: {:?}", res.err());
}

/// 4. Option::as_mut valid write
#[test]
fn test_opt_as_mut_valid_write() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_as_mut_valid_write");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn modify_opt(opt: &rw Option<i32>) {
    dec rw mut_opt = opt.as_mut();
    match mut_opt {
        Option::Some(r) -> *r = 99,
        Option::None -> (),
    }
}

fn main() -> i32 {
    dec rw o = Option::Some(1);
    modify_opt(&rw o);
    match o {
        Option::Some(v) -> { return v; },
        Option::None -> { return 0; },
    }
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Option::as_mut valid write MUST succeed: {:?}", res.err());
}

/// 5. Option::as_mut exclusive borrow conflict rejected
#[test]
fn test_opt_as_mut_exclusive_borrow_conflict_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_as_mut_conflict");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() {
    dec rw opt = Option::Some(10);
    dec m = opt.as_mut();
    dec r = opt.as_ref(); // Cannot take shared borrow while exclusive borrow `m` is active!
    match m {
        Option::Some(v) -> { dec _v1 = *v; },
        Option::None -> (),
    }
    match r {
        Option::Some(v) -> { dec _v2 = *v; },
        Option::None -> (),
    }
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Taking shared borrow while as_mut is active MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("borrowed as &rw") || d.message.contains("Cannot access")),
        "Expected exclusive borrow conflict error, got: {:?}",
        errs
    );
}

/// 6. Result::as_ref and Result::as_mut on Ok and Err branches
#[test]
fn test_res_as_ref_and_as_mut_contract() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("res_as_ref_as_mut");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn check_ok(res: &Result<i32, i32>) -> bool {
    dec r = res.as_ref();
    return r.is_ok();
}

fn check_err(res: &Result<i32, i32>) -> bool {
    dec r = res.as_ref();
    return r.is_err();
}

fn modify_res(res: &rw Result<i32, i32>) {
    dec rw m = res.as_mut();
    match m {
        Result::Ok(val) -> *val = 100,
        Result::Err(err) -> *err = 500,
    }
}

fn main() {
    dec rw r_ok: Result<i32, i32> = Result::Ok(1);
    dec rw r_err: Result<i32, i32> = Result::Err(2);

    dec _c1 = check_ok(&r_ok);
    dec _c2 = check_err(&r_err);

    modify_res(&rw r_ok);
    modify_res(&rw r_err);
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Result as_ref and as_mut contract MUST succeed: {:?}", res.err());
}

/// 7. Returning as_ref() of local Option escapes function scope -> REJECT E3005
#[test]
fn test_opt_res_local_escape_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_res_local_escape");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn escape_opt() -> Option<&i32> {
    dec local_opt = Option::Some(42);
    return local_opt.as_ref();
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Returning as_ref() of local variable MUST be rejected with E3005!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "Expected E3005 LocalBorrowEscape error, got: {:?}",
        errs
    );
}

/// 8. Generic nested projection: Option<Option<&T>>
#[test]
fn test_opt_res_nested_generic_projection() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_res_nested_generic");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn inspect_nested<T>(outer: &Option<Option<T>>) -> bool {
    dec o = outer.as_ref();
    match o {
        Option::Some(inner_ref) -> {
            dec i = inner_ref.as_ref();
            return i.is_some();
        }
        Option::None -> { return false; },
    }
}

fn main() {
    dec nested: Option<Option<i32>> = Option::Some(Option::Some(99));
    dec _r = inspect_nested<i32>(&nested);
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Nested generic Option projection MUST succeed: {:?}", res.err());
}

/// 9. Source .ln vs .llib parity for Option / Result as_ref and as_mut
#[test]
fn test_opt_res_source_and_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_res_llib_parity");
    let lib_src_path = dir.join("my_provider.ln");
    let lib_bin_path = dir.join("my_provider.llib");
    let consumer_path = dir.join("consumer.ln");

    let lib_src = r#"
import <core>;

module my_provider {
    const DUMMY: i32 = 0;

    export fn extract_opt_val(opt: &Option<i32>) -> &i32 life_from(opt) {
        dec r = opt.as_ref();
        match r {
            Option::Some(val) -> { return val; },
            Option::None -> { return &DUMMY; },
        }
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
import "my_provider";

fn main() {
    dec rw opt = Option::Some(100);
    dec v = my_provider::extract_opt_val(&opt);
    opt = Option::Some(200); // Conflict: mutating `opt` while `v` (borrowed from `opt`) is live!
    dec _v = *v;
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
        "Consumer mutating Option while borrow from .llib method is live MUST reject!"
    );
    let errs = res_consumer.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("borrowed") || d.message.contains("Cannot write")),
        "Expected borrow conflict error in consumer, got: {:?}",
        errs
    );
}
