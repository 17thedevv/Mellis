use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_lang_cross_interaction_tests")
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

/// 1. Macro Generating Comptime Block (Step 6 macro expands, Step 8 comptime evaluates)
#[test]
fn test_macro_generating_comptime_block() {
    let src = r#"
macro const_calc {
    (@x: expr, @y: expr) => {
        comptime {
            (@x) * (@y) + 5
        }
    }
}

fn main() -> i32 {
    dec val = const_calc!(10, 4); // 10 * 4 + 5 = 45
    if val != 45 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_macro_generating_comptime", src);
    assert_eq!(exit_code, 0, "test_macro_generating_comptime failed with code {}", exit_code);
}

/// 2. Macro Generating Generic Items (Struct and functions with type parameters)
#[test]
fn test_macro_generating_generic_items() {
    let src = r#"
macro make_pair {
    (@struct_name: ident, @fn_name: ident) => {
        struct @struct_name<T> {
            first: T,
            second: T,
        };

        fn @fn_name<T>(a: T, b: T) -> @struct_name<T> {
            return @struct_name<T> { first: a, second: b };
        }
    }
}

make_pair!(Pair, make_pair_instance);

fn main() -> i32 {
    dec p = make_pair_instance<i32>(100, 250);
    if p.first + p.second != 350 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_macro_generating_generics", src);
    assert_eq!(exit_code, 0, "test_macro_generating_generics failed with code {}", exit_code);
}

/// 3. Type-Independent Comptime Array Length in Generic Function Context
#[test]
fn test_type_independent_comptime_in_generics() {
    let src = r#"
fn make_fixed_array<T>(item: T) -> [T; comptime { 2 * 2 }] {
    dec arr: [T; 4] = [item, item, item, item];
    return arr;
}

fn main() -> i32 {
    dec arr = make_fixed_array<i32>(42);
    if arr[0] != 42 || arr[3] != 42 {
        return 1;
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_type_indep_comptime_generics", src);
    assert_eq!(exit_code, 0, "test_type_indep_comptime_generics failed with code {}", exit_code);
}

/// 4. Monomorphization Boundary: Open generic-dependent comptime is rejected pre-monomorphization (LANGUAGE-GAP)
#[test]
fn test_open_generic_comptime_rejected_pre_mono() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_open_generic_comptime");
    let main_path = temp.join("main.ln");

    let src = r#"
fn generic_val<T>(x: T) -> T {
    return comptime { x };
}

fn main() -> i32 {
    dec v = generic_val<i32>(42);
    return 0;
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
    assert!(compile_res.is_err(), "Expected open generic-dependent comptime to be rejected pre-monomorphization");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_UNRESOLVED_PROJECTION") || d.message.contains("cannot evaluate comptime block")),
        "Expected pre-monomorphization gap diagnostic, got: {:?}",
        errs
    );
}

/// 5. Closures, Borrows, and Loop Control Flow Interactions
#[test]
fn test_closure_borrows_and_loop_control_flow() {
    let src = r#"
fn main() -> i32 {
    dec factor: i32 = 3;
    dec mult = |step: i32| -> i32 {
        return factor * step;
    };

    dec rw total: i32 = 0;
    dec rw i: i32 = 0;
    while i < 10 {
        if i == 5 {
            break;
        }
        total = total + mult(2);
        i = i + 1;
    }

    // 5 iterations * (3 * 2) = 5 * 6 = 30
    if total != 30 {
        return 1;
    }

    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_closure_borrows_loop", src);
    assert_eq!(exit_code, 0, "test_closure_borrows_loop failed with code {}", exit_code);
}
