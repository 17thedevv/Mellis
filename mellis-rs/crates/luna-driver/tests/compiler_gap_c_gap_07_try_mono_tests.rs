use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_07_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_and_run(test_name: &str, source: &str) -> (i32, String, String) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    let exe_path = temp.join(format!("{}.exe", test_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), source.to_string(), &options);
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// C-GAP-07: Test 1
/// Verifies monomorphic ? invocation on Result<i32, i32> succeeds without backend invariant violation.
#[test]
fn test_c_gap_07_monomorphic_try_chain() {
    let src = r#"
import <result>;
import <try>;

fn step1() -> Result<i32, i32> {
    return Result::Ok(10);
}

fn compute() -> Result<i32, i32> {
    dec x = step1()?;
    return Result::Ok(x + 5);
}

fn main() -> i32 {
    dec res = compute();
    match res {
        Result::Ok(v) -> {
            if v == 15 {
                return 0;
            }
            return 1;
        },
        Result::Err(_) -> {
            return 2;
        },
    }
}
"#;
    let (code, _, stderr) = compile_and_run("test_monomorphic_try", src);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}", code, stderr);
}

/// C-GAP-07: Test 2
/// Verifies early exit on Err when using ? operator.
#[test]
fn test_c_gap_07_try_early_return_error() {
    let src = r#"
import <result>;
import <try>;

fn fail_step() -> Result<i32, i32> {
    return Result::Err(42);
}

fn compute() -> Result<i32, i32> {
    dec x = fail_step()?;
    return Result::Ok(x + 100);
}

fn main() -> i32 {
    dec res = compute();
    match res {
        Result::Ok(_) -> {
            return 1;
        },
        Result::Err(e) -> {
            if e == 42 {
                return 0;
            }
            return 2;
        },
    }
}
"#;
    let (code, _, stderr) = compile_and_run("test_try_early_err", src);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}", code, stderr);
}

/// C-GAP-07: Test 3
/// Verifies generic function containing ? operator monomorphizes with concrete types.
#[test]
fn test_c_gap_07_generic_try_caller_callee() {
    let src = r#"
import <result>;
import <try>;

fn unwrap_and_forward<T, E>(val: Result<T, E>) -> Result<T, E> {
    dec inner = val?;
    return Result::Ok(inner);
}

fn main() -> i32 {
    dec ok_in: Result<i32, i32> = Result::Ok(99);
    dec res = unwrap_and_forward(ok_in);
    match res {
        Result::Ok(v) -> {
            if v == 99 {
                return 0;
            }
            return 1;
        },
        Result::Err(_) -> {
            return 2;
        },
    }
}
"#;
    let (code, _, stderr) = compile_and_run("test_generic_try", src);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}", code, stderr);
}

/// C-GAP-07: Test 4
/// Permanent reproducer test reading from tests/sem_stress/c_compiler_gaps.
#[test]
fn test_c_gap_07_permanent_reproducer_file() {
    let reproducer_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/sem_stress/c_compiler_gaps/C-GAP-07_question_operator_generic_substitution.ln");
    assert!(reproducer_path.is_file(), "Reproducer file not found: {:?}", reproducer_path);
    let src = fs::read_to_string(&reproducer_path).unwrap();
    let (code, _, stderr) = compile_and_run("test_c_gap_07_reproducer", &src);
    assert_eq!(code, 0, "Reproducer execution failed with code {}. Stderr: {}", code, stderr);
}
