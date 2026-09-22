use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_lang_comptime_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
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

/// 1. Comptime imperative loops and local variable mutation
#[test]
fn test_comptime_imperative_loops_and_local_mutation() {
    let src = r#"
fn main() -> i32 {
    dec sum = comptime {
        dec rw acc: i32 = 0;
        dec rw i: i32 = 1;
        while i <= 10 {
            acc = acc + i;
            i = i + 1;
        }
        acc
    };

    if sum != 55 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_comptime_loops_mutation", src);
    assert_eq!(exit_code, 0, "test_comptime_loops_mutation failed with code {}", exit_code);
}

/// 2. Comptime recursive functions and constant binding
#[test]
fn test_comptime_recursion_and_pure_fn_calls() {
    let src = r#"
fn factorial(n: i32) -> i32 {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

const FACT_6: i32 = comptime { factorial(6) }; // 720

fn main() -> i32 {
    if FACT_6 != 720 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_comptime_recursion", src);
    assert_eq!(exit_code, 0, "test_comptime_recursion failed with code {}", exit_code);
}

/// 3. Comptime struct construction, fields and composite values
#[test]
fn test_comptime_struct_and_composite_values() {
    let src = r#"
struct Point {
    x: i32,
    y: i32,
};

const ORIGIN_DIST_SQ: i32 = comptime {
    dec p = Point { x: 12, y: 5 };
    p.x * p.x + p.y * p.y
};

fn main() -> i32 {
    if ORIGIN_DIST_SQ != 169 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_comptime_struct_values", src);
    assert_eq!(exit_code, 0, "test_comptime_struct_values failed with code {}", exit_code);
}

/// 4. Array length sized by comptime block expression
#[test]
fn test_comptime_array_length_expression() {
    let src = r#"
fn main() -> i32 {
    dec arr: [i32; comptime { 2 + 2 }] = [10, 20, 30, 40];
    if arr[0] + arr[3] != 50 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_comptime_array_length", src);
    assert_eq!(exit_code, 0, "test_comptime_array_length failed with code {}", exit_code);
}

/// 5. Negative assertion: Division by zero at compile time
#[test]
fn test_comptime_division_by_zero_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_comptime_div_zero");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec val = comptime {
        100 / 0
    };
    return val;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected compile-time division by zero to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("divide by zero") || d.message.contains("cannot evaluate comptime block")),
        "Expected division by zero diagnostic, got: {:?}",
        errs
    );
}

/// 6. Negative assertion: Step limit exceeded in infinite loop
#[test]
fn test_comptime_step_limit_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_comptime_step_limit");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec val = comptime {
        dec rw i: i32 = 0;
        while true {
            i = i + 1;
        }
        i
    };
    return val;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        comptime_steps: Some(10_000), // Reduce steps for faster test execution
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected comptime step limit exceeded to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("step limit exceeded") || d.message.contains("cannot evaluate comptime block")),
        "Expected step limit exceeded diagnostic, got: {:?}",
        errs
    );
}

/// 7. Negative assertion: Recursion depth limit exceeded
#[test]
fn test_comptime_recursion_limit_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_comptime_recursion_limit");
    let main_path = temp.join("main.ln");

    let src = r#"
fn infinite_call(x: i32) -> i32 {
    return infinite_call(x + 1);
}

fn main() -> i32 {
    dec val = comptime {
        infinite_call(0)
    };
    return val;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        comptime_depth: Some(50), // Reduce depth for faster test execution
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected comptime recursion depth limit exceeded to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("recursion limit exceeded") || d.message.contains("cannot evaluate comptime block")),
        "Expected recursion limit exceeded diagnostic, got: {:?}",
        errs
    );
}

/// 8. Negative assertion: Forbidden side effects (extern fn call in comptime)
#[test]
fn test_comptime_forbidden_side_effect_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_comptime_side_effect");
    let main_path = temp.join("main.ln");

    let src = r#"
extern fn foreign_func(x: i32) -> i32;

fn main() -> i32 {
    dec val = comptime {
        foreign_func(42)
    };
    return val;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected extern function call in comptime to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("forbidden") || d.message.contains("cannot evaluate comptime block")),
        "Expected forbidden side-effect diagnostic, got: {:?}",
        errs
    );
}

/// 9. Comptime vs Runtime Parity
#[test]
fn test_comptime_vs_runtime_parity() {
    let src = r#"
fn collatz_steps(start: i32) -> i32 {
    dec rw n = start;
    dec rw steps = 0;
    while n > 1 {
        if n % 2 == 0 {
            n = n / 2;
        } else {
            n = 3 * n + 1;
        }
        steps = steps + 1;
    }
    return steps;
}

const COMPTIME_STEPS: i32 = comptime { collatz_steps(27) };

fn main() -> i32 {
    dec runtime_steps = collatz_steps(27);
    if COMPTIME_STEPS != runtime_steps {
        return 1;
    }
    // Collatz of 27 is known to take 111 steps
    if COMPTIME_STEPS != 111 {
        return 2;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_comptime_runtime_parity", src);
    assert_eq!(exit_code, 0, "test_comptime_runtime_parity failed with code {}", exit_code);
}
