use luna_driver::{CompilerOptions, compile};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_main_shim_tests").join(name);
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

#[test]
fn test_main_void_normalizes_to_zero() {
    let src = r#"
        fn main() {}
    "#;
    let (code, _, _) = compile_and_run("test_main_void_normalizes_to_zero", src);
    assert_eq!(code, 0, "fn main() must normalize exit code to 0");
}

#[test]
fn test_main_explicit_void_normalizes_to_zero() {
    let src = r#"
        fn main() -> void {}
    "#;
    let (code, _, _) = compile_and_run("test_main_explicit_void_normalizes_to_zero", src);
    assert_eq!(code, 0, "fn main() -> void must normalize exit code to 0");
}

#[test]
fn test_main_i32_exit_code_propagation() {
    let src = r#"
        fn main() -> i32 {
            return 42;
        }
    "#;
    let (code, _, _) = compile_and_run("test_main_i32_exit_code_propagation", src);
    assert_eq!(code, 42, "fn main() -> i32 must propagate return value as process exit code");
}

#[test]
fn test_main_args_slice_exit_code() {
    let src = r#"
        fn main(args: [str]) -> i32 {
            return 7;
        }
    "#;
    let (code, _, _) = compile_and_run("test_main_args_slice_exit_code", src);
    assert_eq!(code, 7, "fn main(args: [str]) -> i32 must execute through shim and return exit code");
}

#[test]
fn test_native_bounds_check_triggers_runtime_trap() {
    let src = r#"
        fn main() -> i32 {
            dec arr: [i32; 3] = [10, 20, 30];
            dec x = arr[5];
            return x;
        }
    "#;
    let (code, _, stderr) = compile_and_run("test_native_bounds_check_triggers_runtime_trap", src);
    assert_ne!(code, 0, "Out-of-bounds access at runtime must terminate abnormally");
    assert!(
        stderr.contains("index out of bounds") && stderr.contains("index 5, length 3"),
        "Expected stderr to contain bounds failure diagnostic, got: {}",
        stderr
    );
}
