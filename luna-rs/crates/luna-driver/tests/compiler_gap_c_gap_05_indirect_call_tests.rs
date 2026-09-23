use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_05_{}", name));
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

/// C-GAP-05: Test 1
/// Verifies indirect function pointer call returning integer value.
#[test]
fn test_c_gap_05_indirect_call_value_return() {
    let src = r#"
fn square(x: i32) -> i32 {
    return x * x;
}

fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    return (f)(x);
}

fn main() -> i32 {
    dec res = apply(square, 7);
    if res == 49 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("value_return", src);
    assert_eq!(exit_code, 0, "Indirect call with value return failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-05: Test 2
/// Verifies indirect function pointer call returning void (unit struct) without i32 0 fallback.
#[test]
fn test_c_gap_05_indirect_call_void_return() {
    let src = r#"
fn increment(ptr: *rw i32) {
    unsafe {
        *ptr = *ptr + 10;
    }
}

fn apply_effect(action: fn(*rw i32), target: *rw i32) {
    (action)(target);
}

fn main() -> i32 {
    dec rw val: i32 = 5;
    apply_effect(increment, &rw val as *rw i32);
    if val == 15 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("void_return", src);
    assert_eq!(exit_code, 0, "Indirect call with void return failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-05: Test 3
/// Verifies indirect function pointer stored inside a struct field and invoked.
#[test]
fn test_c_gap_05_struct_field_indirect_call() {
    let src = r#"
struct MathEngine {
    op: fn(i32, i32) -> i32,
    bias: i32,
};

fn multiply(a: i32, b: i32) -> i32 {
    return a * b;
}

fn main() -> i32 {
    dec engine = MathEngine {
        op: multiply,
        bias: 3,
    };
    dec result = (engine.op)(6, 7) + engine.bias;
    if result == 45 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("struct_field_indirect", src);
    assert_eq!(exit_code, 0, "Struct field indirect call failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-05: Test 4
/// Verifies that CallDirect and CallVirt adhere to the same non-void semantic integrity invariant
/// (non-void returns must never silently degrade to 0).
#[test]
fn test_c_gap_05_direct_and_virtual_call_semantic_integrity() {
    let src = r#"
trait Evaluator {
    fn compute(self: &Self, v: i32) -> i32;
}

struct DoubleEvaluator {
    multiplier: i32,
};

impl Evaluator for DoubleEvaluator {
    fn compute(self: &Self, v: i32) -> i32 {
        return v * self.multiplier;
    }
}

fn direct_helper(x: i32) -> i32 {
    return x + 5;
}

fn run_dyn(eval: &dyn Evaluator, val: i32) -> i32 {
    return eval.compute(val);
}

fn main() -> i32 {
    // 1. Direct call verification
    dec d = direct_helper(10);
    if d != 15 {
        return 1;
    }

    // 2. Virtual call verification
    dec obj = DoubleEvaluator { multiplier: 3 };
    dec v = run_dyn(&obj, 7);
    if v != 21 {
        return 2;
    }

    return 0;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("direct_and_virt_integrity", src);
    assert_eq!(exit_code, 0, "Direct and virtual call semantic integrity test failed: stdout={}, stderr={}", stdout, stderr);
}

