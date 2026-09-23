use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_p4b_comb_{}_{}_{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

fn run_binary(dir: &Path, src: &str, opts: &CompilerOptions) -> Result<(i32, String, String), String> {
    let src_path = dir.join("main.ln");
    let exe_path = dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    fs::write(&src_path, src).map_err(|e| e.to_string())?;

    let mut compile_opts = opts.clone();
    compile_opts.output_path = Some(exe_path.to_str().unwrap().to_string());

    let compile_res = compile(src_path.to_str().unwrap(), src.to_string(), &compile_opts);
    if let Err(err) = compile_res {
        return Err(format!("Compilation failed: {:?}", err));
    }

    let output = Command::new(&exe_path)
        .output()
        .map_err(|e| format!("Execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((code, stdout, stderr))
}

#[test]
fn test_option_map_and_and_then() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("opt_map");

    let src = r#"
        fn double_it(x: i32) -> i32 {
            return x * 2;
        }

        fn checked_recip(x: i32) -> Option<i32> {
            if x == 0 {
                return Option::None;
            }
            return Option::Some(100 / x);
        }

        fn main() -> i32 {
            // Test map
            dec some_val1: Option<i32> = Option::Some(10);
            dec m1 = some_val1.map<i32>(double_it);
            if m1.unwrap_or(0) != 20 { return 1; }

            dec none_val1: Option<i32> = Option::None;
            dec m2 = none_val1.map<i32>(double_it);
            if m2.is_some() == true { return 2; }

            // Test and_then
            dec some_val2: Option<i32> = Option::Some(10);
            dec at1 = some_val2.and_then<i32>(checked_recip);
            if at1.unwrap_or(0) != 10 { return 3; }

            dec zero_val: Option<i32> = Option::Some(0);
            dec at2 = zero_val.and_then<i32>(checked_recip);
            if at2.is_some() == true { return 4; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_option_unwrap_or_else_and_or_else() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("opt_or_else");

    let src = r#"
        fn fallback_val() -> i32 {
            return 42;
        }

        fn fallback_opt() -> Option<i32> {
            return Option::Some(99);
        }

        fn main() -> i32 {
            // unwrap_or_else
            dec s1: Option<i32> = Option::Some(7);
            dec n1: Option<i32> = Option::None;
            if s1.unwrap_or_else(fallback_val) != 7 { return 1; }
            if n1.unwrap_or_else(fallback_val) != 42 { return 2; }

            // or_else
            dec s2: Option<i32> = Option::Some(7);
            dec n2: Option<i32> = Option::None;
            dec o1 = s2.or_else(fallback_opt);
            if o1.unwrap_or(0) != 7 { return 3; }

            dec o2 = n2.or_else(fallback_opt);
            if o2.unwrap_or(0) != 99 { return 4; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_option_filter() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("opt_filter");

    let src = r#"
        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn main() -> i32 {
            dec even_val: Option<i32> = Option::Some(4);
            dec odd_val: Option<i32> = Option::Some(7);
            dec none_val: Option<i32> = Option::None;

            dec f1 = even_val.filter(is_even);
            if f1.is_some() == false { return 1; }
            if f1.unwrap_or(0) != 4 { return 2; }

            dec f2 = odd_val.filter(is_even);
            if f2.is_some() == true { return 3; }

            dec f3 = none_val.filter(is_even);
            if f3.is_some() == true { return 4; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_result_map_and_map_err() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("res_map");

    let src = r#"
        import <result>;

        fn add_ten(x: i32) -> i32 {
            return x + 10;
        }

        fn negate_err(e: i32) -> i32 {
            return 0 - e;
        }

        fn main() -> i32 {
            // map
            dec ok_val1: Result<i32, i32> = Result::Ok(5);
            dec err_val1: Result<i32, i32> = Result::Err(3);

            dec m_ok = ok_val1.map<i32>(add_ten);
            if m_ok.is_ok() == false { return 1; }
            if m_ok.unwrap() != 15 { return 2; }

            dec m_err = err_val1.map<i32>(add_ten);
            if m_err.is_ok() == true { return 3; }
            if m_err.is_err() == false { return 4; }
            dec opt_err1 = m_err.err();
            if opt_err1.unwrap() != 3 { return 5; }

            // map_err
            dec ok_val2: Result<i32, i32> = Result::Ok(5);
            dec err_val2: Result<i32, i32> = Result::Err(3);

            dec me_ok = ok_val2.map_err<i32>(negate_err);
            if me_ok.is_ok() == false { return 6; }
            if me_ok.unwrap() != 5 { return 7; }

            dec me_err = err_val2.map_err<i32>(negate_err);
            if me_err.is_ok() == true { return 8; }
            if me_err.is_err() == false { return 9; }
            dec opt_err2 = me_err.err();
            if opt_err2.unwrap() != (0 - 3) { return 10; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_result_and_then_and_unwrap_or_else() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("res_and_then");

    let src = r#"
        import <result>;

        fn parse_positive(x: i32) -> Result<i32, i32> {
            if x > 0 {
                return Result::Ok(x * 2);
            }
            return Result::Err(0 - 1);
        }

        fn handle_err(e: i32) -> i32 {
            return 100 + e;
        }

        fn main() -> i32 {
            dec ok_val: Result<i32, i32> = Result::Ok(5);
            dec zero_val: Result<i32, i32> = Result::Ok(0);
            dec err_val: Result<i32, i32> = Result::Err(50);

            // and_then Ok -> Ok
            dec at1 = ok_val.and_then<i32>(parse_positive);
            if at1.is_ok() == false { return 1; }
            if at1.unwrap() != 10 { return 2; }

            // and_then Ok -> Err
            dec at2 = zero_val.and_then<i32>(parse_positive);
            if at2.is_ok() == true { return 3; }
            dec opt_err1 = at2.err();
            if opt_err1.unwrap() != (0 - 1) { return 4; }

            // and_then Err -> Err
            dec at3 = err_val.and_then<i32>(parse_positive);
            if at3.is_ok() == true { return 5; }
            dec opt_err2 = at3.err();
            if opt_err2.unwrap() != 50 { return 6; }

            // unwrap_or_else
            dec ok_val2: Result<i32, i32> = Result::Ok(77);
            dec err_val2: Result<i32, i32> = Result::Err(5);
            if ok_val2.unwrap_or_else(handle_err) != 77 { return 7; }
            if err_val2.unwrap_or_else(handle_err) != 105 { return 8; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}
