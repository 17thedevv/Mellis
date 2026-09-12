use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_stdlib_01b_acceptance_tests")
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

/// 1. Option::ok_or with Some returns Ok(val)
#[test]
fn test_opt_ok_or_some() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_ok_or_some");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() -> i32 {
    dec opt = Option::Some(42);
    dec res = opt.ok_or(0);
    return match res {
        Result::Ok(val) -> val,
        Result::Err(_) -> 0,
    };
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Option::ok_or with Some MUST succeed: {:?}", res.err());
}

/// 2. Option::ok_or with None returns Err(err)
#[test]
fn test_opt_ok_or_none() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_ok_or_none");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() -> i32 {
    dec opt: Option<i32> = Option::None;
    dec res = opt.ok_or(99);
    return match res {
        Result::Ok(_) -> 0,
        Result::Err(e) -> e,
    };
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Option::ok_or with None MUST succeed: {:?}", res.err());
}

/// 3. Result::ok converts Ok -> Some and Err -> None
#[test]
fn test_res_ok_from_ok_and_err() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("res_ok_from_ok_and_err");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn test_ok() -> i32 {
    dec r: Result<i32, i32> = Result::Ok(10);
    dec o = r.ok();
    return match o {
        Option::Some(val) -> val,
        Option::None -> 0,
    };
}

fn test_err() -> i32 {
    dec r: Result<i32, i32> = Result::Err(20);
    dec o = r.ok();
    return match o {
        Option::Some(_) -> 0,
        Option::None -> 1,
    };
}

fn main() -> i32 {
    return test_ok() + test_err();
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Result::ok MUST succeed: {:?}", res.err());
}

/// 4. Result::err converts Err -> Some and Ok -> None
#[test]
fn test_res_err_from_err_and_ok() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("res_err_from_err_and_ok");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn test_err() -> i32 {
    dec r: Result<i32, i32> = Result::Err(55);
    dec o = r.err();
    return match o {
        Option::Some(e) -> e,
        Option::None -> 0,
    };
}

fn test_ok() -> i32 {
    dec r: Result<i32, i32> = Result::Ok(66);
    dec o = r.err();
    return match o {
        Option::Some(_) -> 0,
        Option::None -> 1,
    };
}

fn main() -> i32 {
    return test_err() + test_ok();
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Result::err MUST succeed: {:?}", res.err());
}

/// 5. Option::ok consuming identity
#[test]
fn test_opt_ok_identity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("opt_ok_identity");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

fn main() -> i32 {
    dec o = Option::Some(99);
    dec o2 = o.ok();
    return match o2 {
        Option::Some(val) -> val,
        Option::None -> 0,
    };
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Option::ok consuming identity MUST succeed: {:?}", res.err());
}

/// 6. Consuming move invalidates receiver (Negative test)
#[test]
fn test_consuming_move_invalidates_source() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("consuming_move_invalidates");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct NoCopy { val: i32 }

fn main() {
    dec opt = Option::Some(NoCopy { val: 1 });
    dec res = opt.ok_or(0);
    dec _again = opt; // Error: use of moved value
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Accessing moved Option MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("moved value") || d.message.contains("Cannot move")),
        "Expected move error, got: {:?}",
        errs
    );
}

/// 7. Result::ok consuming move invalidates receiver (Negative test)
#[test]
fn test_result_ok_consuming_move_invalidates_source() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("result_ok_consuming_move_invalidates");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct NoCopy { val: i32 }

fn main() {
    dec res: Result<NoCopy, i32> = Result::Ok(NoCopy { val: 1 });
    dec opt = res.ok();
    dec _again = res; // Error: use of moved value
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Accessing moved Result MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("moved value") || d.message.contains("Cannot move")),
        "Expected move error, got: {:?}",
        errs
    );
}

/// 8. Drop payload destruction cleanliness (no partial-move-under-drop)
#[test]
fn test_drop_payload_destruction_cleanliness() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("drop_payload_destruction");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct Droppable { id: i32 }
impl Drop for Droppable {
    fn drop(self: &rw Self) {}
}

fn test_opt_drop() -> i32 {
    dec opt = Option::Some(Droppable { id: 1 });
    dec res = opt.ok_or(99);
    return 0;
}

fn test_res_drop() -> i32 {
    dec res: Result<Droppable, i32> = Result::Ok(Droppable { id: 2 });
    dec opt = res.ok();
    return 0;
}

fn main() -> i32 {
    test_opt_drop();
    test_res_drop();
    return 0;
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Conversion with Drop payloads MUST succeed cleanly: {:?}", res.err());
}

/// 9. Generic error type inference on ok_or
#[test]
fn test_generic_error_inference() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("generic_error_inference");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct CustomError { code: i32 }

fn main() -> i32 {
    dec opt1 = Option::Some(10);
    dec res1 = opt1.ok_or(100);

    dec opt2: Option<i32> = Option::None;
    dec res2 = opt2.ok_or(CustomError { code: 404 });

    return 0;
}
"#;
    let path = dir.join("main.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Generic error type inference MUST succeed: {:?}", res.err());
}

/// 10. Source .ln vs .llib parity for conversions
#[test]
fn test_source_and_llib_conversions_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("conversions_llib_parity");
    let lib_src_path = dir.join("conv_provider.ln");
    let lib_bin_path = dir.join("conv_provider.llib");
    let consumer_path = dir.join("consumer.ln");

    let lib_src = r#"
import <core>;

module conv_provider {
    export fn to_res(opt: Option<i32>, default_err: i32) -> Result<i32, i32> {
        return opt.ok_or(default_err);
    }

    export fn to_opt(res: Result<i32, i32>) -> Option<i32> {
        return res.ok();
    }

    export fn to_err_opt(res: Result<i32, i32>) -> Option<i32> {
        return res.err();
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
    assert!(res_compile.is_ok(), "Compiling conv_provider.ln to .llib MUST succeed: {:?}", res_compile.err());

    let consumer_src = r#"
import <core>;
import "conv_provider";

fn main() -> i32 {
    dec opt = Option::Some(123);
    dec res = conv_provider::to_res(opt, 0);
    dec back_opt = conv_provider::to_opt(res);
    return match back_opt {
        Option::Some(v) -> v,
        Option::None -> 0,
    };
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
    assert!(res_consumer.is_ok(), "Consumer using .llib conversions MUST succeed: {:?}", res_consumer.err());
}
