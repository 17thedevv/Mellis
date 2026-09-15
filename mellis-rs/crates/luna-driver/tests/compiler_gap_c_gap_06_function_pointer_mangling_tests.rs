use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_06_{}", name));
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

/// C-GAP-06: Test 1
/// Verifies that top-level function passed as function pointer uses canonical instance mangling.
#[test]
fn test_c_gap_06_canonical_function_pointer_value() {
    let src = r#"
fn add_three(x: i32) -> i32 {
    return x + 3;
}

fn execute(f: fn(i32) -> i32, arg: i32) -> i32 {
    return (f)(arg);
}

fn main() -> i32 {
    dec f_ptr = add_three;
    dec res = execute(f_ptr, 10);
    if res == 13 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("canon_fp_val", src);
    assert_eq!(exit_code, 0, "Canonical function pointer value test failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-06: Test 2
/// Verifies passing two distinct functions with identical local signature as function pointers.
#[test]
fn test_c_gap_06_distinct_function_pointers() {
    let src = r#"
fn op_add(x: i32) -> i32 {
    return x + 100;
}

fn op_sub(x: i32) -> i32 {
    return x - 50;
}

fn dispatch(condition: bool) -> fn(i32) -> i32 {
    if condition {
        return op_add;
    }
    return op_sub;
}

fn main() -> i32 {
    dec f1 = dispatch(true);
    dec f2 = dispatch(false);
    dec r1 = (f1)(10);  // 110
    dec r2 = (f2)(10);  // -40
    if r1 == 110 && r2 == -40 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("distinct_fp", src);
    assert_eq!(exit_code, 0, "Distinct function pointer test failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-06-A:
/// Verifies that a generic function value instantiated with concrete type arguments
/// (e.g. `identity<i32>`) is correctly lowered to its canonical monomorphized symbol.
#[test]
fn test_c_gap_06_a_generic_function_pointer_value() {
    let src = r#"
fn identity<T>(x: T) -> T {
    return x;
}

fn apply_int(f: fn(i32) -> i32, x: i32) -> i32 {
    return (f)(x);
}

fn main() -> i32 {
    dec f = identity<i32>;
    dec res = apply_int(f, 99);
    if res == 99 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("generic_fp_val", src);
    assert_eq!(exit_code, 0, "Generic function pointer value test failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-06-B:
/// Verifies that the same generic function instantiated with different substitutions
/// (`identity<i32>` and `identity<i64>`) produces distinct canonical instances and symbols
/// with correct runtime execution.
#[test]
fn test_c_gap_06_b_distinct_generic_substitutions() {
    let src = r#"
fn identity<T>(x: T) -> T {
    return x;
}

fn apply_i32(f: fn(i32) -> i32, x: i32) -> i32 {
    return (f)(x);
}

fn apply_i64(f: fn(i64) -> i64, x: i64) -> i64 {
    return (f)(x);
}

fn main() -> i32 {
    dec f_a = identity<i32>;
    dec f_b = identity<i64>;
    dec r1 = apply_i32(f_a, 42);
    dec r2 = apply_i64(f_b, 100 as i64);
    if r1 == 42 && r2 == (100 as i64) {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("distinct_generic_substs", src);
    assert_eq!(exit_code, 0, "Distinct generic substitutions test failed: stdout={}, stderr={}", stdout, stderr);
}

