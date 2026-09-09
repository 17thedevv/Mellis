use mellis_driver::{CompilerOptions, check};
use mellis_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_main_entrypoint_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn check_source(name: &str, source: &str) -> Result<(), Vec<mellis_common::Diagnostic>> {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ms");
    fs::write(&main_path, source).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    check(main_path.to_str().unwrap(), source.to_string(), &options)
}

#[test]
fn test_main_valid_args() {
    let src = r#"
        fn main(args: [str]) -> i32 {
            dec len = args.length;
            dec first = args[0];
            return 0;
        }
    "#;
    assert!(check_source("test_main_valid_args", src).is_ok(), "{:?}", check_source("test_main_valid_args", src));
}

#[test]
fn test_main_valid_no_args_i32() {
    let src = r#"
        fn main() -> i32 {
            return 0;
        }
    "#;
    assert!(check_source("test_main_valid_no_args_i32", src).is_ok(), "{:?}", check_source("test_main_valid_no_args_i32", src));
}

#[test]
fn test_main_valid_no_args_void() {
    let src = r#"
        fn main() {}
    "#;
    assert!(check_source("test_main_valid_no_args_void", src).is_ok());
}

#[test]
fn test_main_invalid_param_type() {
    let src = r#"
        fn main(x: i32) -> i32 {
            return 0;
        }
    "#;
    let errs = check_source("test_main_invalid_param_type", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_main_invalid_param_borrow() {
    let src = r#"
        fn main(args: &[str]) -> i32 {
            return 0;
        }
    "#;
    let errs = check_source("test_main_invalid_param_borrow", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_main_invalid_param_array() {
    let src = r#"
        fn main(args: [str; 10]) -> i32 {
            return 0;
        }
    "#;
    let errs = check_source("test_main_invalid_param_array", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_main_invalid_return() {
    let src = r#"
        fn main(args: [str]) -> bool {
            return true;
        }
    "#;
    let errs = check_source("test_main_invalid_return", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_main_invalid_args_void() {
    let src = r#"
        fn main(args: [str]) {
        }
    "#;
    let errs = check_source("test_main_invalid_args_void", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_main_invalid_async() {
    let src = r#"
        async fn main() -> i32 {
            return 0;
        }
    "#;
    let errs = check_source("test_main_invalid_async", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_INVALID_MAIN_SIGNATURE")));
}

#[test]
fn test_ordinary_fn_unsized_args() {
    let src = r#"
        fn ordinary(args: [str]) -> i32 {
            return 0;
        }
    "#;
    let errs = check_source("test_ordinary_fn_unsized_args", src).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION")));
}
