use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_lang_unsafe_ffi_tests")
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

/// 1. Raw pointer element-stride pointer arithmetic and mutable dereference inside unsafe
#[test]
fn test_unsafe_raw_pointer_element_stride_arithmetic() {
    let src = r#"
fn main() -> i32 {
    dec rw arr: [i32; 4] = [10, 20, 30, 40];
    dec base: *rw i32 = &rw arr[0] as *rw i32;

    unsafe {
        dec p1 = base + 1; // Strides to arr[1]
        dec p2 = base + 2; // Strides to arr[2]
        dec p3 = base + 3; // Strides to arr[3]

        if *p1 != 20 {
            return 1;
        }
        if *p2 != 30 {
            return 2;
        }
        if *p3 != 40 {
            return 3;
        }

        // Mutation through mutable raw pointer
        *p2 = 999;
    }

    if arr[2] != 999 {
        return 4;
    }

    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_unsafe_pointer_arith", src);
    assert_eq!(exit_code, 0, "test_unsafe_pointer_arith failed with code {}", exit_code);
}

/// 2. Raw pointer difference (distance in elements) inside unsafe
#[test]
fn test_unsafe_raw_pointer_distance() {
    let src = r#"
fn main() -> i32 {
    dec arr: [i32; 5] = [1, 2, 3, 4, 5];
    dec p0: *i32 = &arr[0] as *i32;
    dec p4: *i32 = &arr[4] as *i32;

    unsafe {
        dec diff: isize = p4 - p0;
        if (diff as i32) != 4 {
            return 1;
        }
    }
    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_unsafe_pointer_diff", src);
    assert_eq!(exit_code, 0, "test_unsafe_pointer_diff failed with code {}", exit_code);
}

/// 3. Negative assertion: Raw pointer dereference outside unsafe is rejected
#[test]
fn test_unsafe_deref_outside_unsafe_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_deref_outside_unsafe");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec rw val = 10;
    dec p: *rw i32 = &rw val as *rw i32;
    *p = 42; // Rejected: deref outside unsafe
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
    assert!(compile_res.is_err(), "Expected raw pointer deref outside unsafe to fail");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_UNSAFE_DEREF_OUTSIDE_UNSAFE") || d.message.contains("requires an unsafe block")),
        "Expected unsafe deref diagnostic, got: {:?}",
        errs
    );
}

/// 4. Negative assertion: Pointer arithmetic outside unsafe is rejected
#[test]
fn test_unsafe_pointer_arithmetic_outside_unsafe_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_arith_outside_unsafe");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec arr: [i32; 2] = [1, 2];
    dec p: *i32 = &arr[0] as *i32;
    dec p2 = p + 1; // Rejected: pointer arithmetic outside unsafe
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
    assert!(compile_res.is_err(), "Expected pointer arithmetic outside unsafe to fail");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE") || d.message.contains("requires an unsafe block")),
        "Expected pointer arithmetic outside unsafe diagnostic, got: {:?}",
        errs
    );
}

/// 5. Negative assertion: Mutation through immutable raw pointer *T is rejected even inside unsafe
#[test]
fn test_unsafe_mutate_through_immutable_pointer_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_mutate_immutable_ptr");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec val = 42;
    dec p: *i32 = &val as *i32;
    unsafe {
        *p = 100; // Rejected: cannot assign through immutable raw pointer '*T'
    }
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
    assert!(compile_res.is_err(), "Expected assignment through *T to fail");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_CANNOT_MUTATE_IMMUTABLE_POINTER") || d.message.contains("immutable raw pointer")),
        "Expected immutable raw pointer mutation diagnostic, got: {:?}",
        errs
    );
}

/// 6. Negative assertion: Unsafe semantic containment — Borrow checker remains active inside unsafe
#[test]
fn test_unsafe_containment_borrowck_still_active() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_unsafe_containment_borrowck");
    let main_path = temp.join("main.ln");

    let src = r#"
fn main() -> i32 {
    dec rw x = 10;
    unsafe {
        dec r1 = &rw x;
        dec r2 = &rw x; // Rejected: Aliasing mutable borrow inside unsafe!
        *r1 = 20;
        *r2 = 30;
    }
    return x;
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
    assert!(compile_res.is_err(), "Expected borrowck violation inside unsafe to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("borrow") || d.message.contains("cannot borrow")),
        "Expected borrowck diagnostic inside unsafe, got: {:?}",
        errs
    );
}

/// 7. FFI Contract: Struct with #[repr(C)] crossing FFI boundary is accepted
#[test]
fn test_ffi_safe_c_struct_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_ffi_repr_c_accepted");
    let main_path = temp.join("main.ln");

    let src = r#"
#[repr(C)]
struct CPoint {
    x: i32,
    y: i32,
};

extern fn compute_distance(pt: CPoint) -> i32;

fn main() -> i32 {
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
    assert!(compile_res.is_ok(), "Expected #[repr(C)] struct in extern fn to pass: {:?}", compile_res.err());
}

/// 8. Negative assertion: Struct without #[repr(C)] crossing FFI boundary is rejected (E2030)
#[test]
fn test_ffi_struct_without_repr_c_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_ffi_no_repr_c");
    let main_path = temp.join("main.ln");

    let src = r#"
struct LunaOnlyStruct {
    x: i32,
    y: i32,
};

extern fn bad_extern_call(s: LunaOnlyStruct) -> i32;

fn main() -> i32 {
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
    assert!(compile_res.is_err(), "Expected struct without #[repr(C)] in extern fn to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2030") || d.message.contains("repr(C)")),
        "Expected E2030 repr(C) diagnostic, got: {:?}",
        errs
    );
}

/// 9. Negative assertion: By-value aggregate containing pointer capability rejected across FFI (E2030)
#[test]
fn test_ffi_aggregate_with_pointer_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_ffi_ptr_aggregate");
    let main_path = temp.join("main.ln");

    let src = r#"
#[repr(C)]
struct StructWithPointer {
    ptr: *i32,
};

extern fn dangerous_ffi(s: StructWithPointer) -> i32;

fn main() -> i32 {
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
    assert!(compile_res.is_err(), "Expected aggregate with pointer capability across FFI to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2030") || d.message.contains("pointer capability")),
        "Expected E2030 pointer capability diagnostic, got: {:?}",
        errs
    );
}

/// 10. Negative assertion: Extern function returning safe reference without explicit contract is rejected (E2030)
#[test]
fn test_ffi_returning_safe_ref_without_contract_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_ffi_safe_ref_return");
    let main_path = temp.join("main.ln");

    let src = r#"
extern fn get_reference(x: i32) -> &i32;

fn main() -> i32 {
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
    assert!(compile_res.is_err(), "Expected extern function returning safe reference without contract to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2030") || d.message.contains("lifetime contract")),
        "Expected E2030 safe reference return diagnostic, got: {:?}",
        errs
    );
}
