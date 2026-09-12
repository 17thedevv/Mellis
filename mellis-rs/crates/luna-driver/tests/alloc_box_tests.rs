/// Alloc + Box<T> Integration Tests
///
/// Verifies that the alloc library source files (`box.ln`, `allocator.ln`, `vec.ln`)
/// pass through the full compilation pipeline: parse → semantic → MVIR → LLVM.
///
/// These are not yet E2E binary execution tests (that requires `alloc` to be
/// registered as an external component in the test sysroot). Instead, they verify:
/// 1. The library sources are syntactically and semantically valid.
/// 2. MVIR generation succeeds without ICE.
/// 3. Programs using alloc primitives compile through check().

use std::fs;
use std::path::PathBuf;
use luna_driver::{check, check_semantic_only, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn test_sysroot_options() -> (Sysroot, CompilerOptions) {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    (sysroot, options)
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("luna_alloc_box_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// =============================================================================
// Phase 1: Verify alloc library source files are valid Luna programs
// =============================================================================

/// Verify that `allocator.ln` passes semantic check when `core` is available.
#[test]
fn test_allocator_ln_is_valid() {
    let (_sysroot, options) = test_sysroot_options();
    let lib_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("lib")
        .join("alloc");
    let alloc_path = lib_dir.join("allocator.ln");
    if !alloc_path.exists() {
        eprintln!("SKIP: allocator.ln not found at {:?}", alloc_path);
        return;
    }
    let source = fs::read_to_string(&alloc_path).unwrap();
    let result = check_semantic_only(alloc_path.to_str().unwrap(), source, &options);
    assert!(
        result.is_ok(),
        "allocator.ln failed semantic check: {:?}",
        result.unwrap_err()
    );
}

/// Verify that `box.ln` passes semantic check when `core` is available.
/// Note: box.ln depends on __mellis_alloc/__mellis_dealloc which are normally
/// provided by allocator.ln. We prepend extern declarations so the file
/// can be checked in isolation.
#[test]
fn test_box_ln_is_valid() {
    let (_sysroot, options) = test_sysroot_options();
    let lib_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("lib")
        .join("alloc");
    let box_path = lib_dir.join("box.ln");
    if !box_path.exists() {
        eprintln!("SKIP: box.ln not found at {:?}", box_path);
        return;
    }
    let raw_source = fs::read_to_string(&box_path).unwrap();
    // Prepend the FFI declarations that box.ln expects from allocator.ln
    let preamble = r#"
export extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
export extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);
"#;
    let source = format!("{}{}", preamble, raw_source);
    let result = check_semantic_only(box_path.to_str().unwrap(), source, &options);
    assert!(
        result.is_ok(),
        "box.ln failed semantic check: {:?}",
        result.unwrap_err()
    );
}

// =============================================================================
// Phase 2: Verify programs using Box<T> primitives compile
// =============================================================================

/// A program that creates a Box<T> via compiler-primitive syntax should pass
/// through semantic analysis + MVIR + LLVM without errors.
#[test]
fn test_box_type_recognized_in_function_signature() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("box_type_sig");
    let main_path = temp.join("main.ln");
    let src = r#"
        fn box_identity(b: Box<i32>) -> Box<i32> {
            return b;
        }
        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "Box<T> in function signature should be recognized. Diags: {:?}",
        result.unwrap_err()
    );
}

/// Verify that Box<T> can be used with nested generic types.
#[test]
fn test_box_nested_generic_type() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("box_nested_generic");
    let main_path = temp.join("main.ln");
    let src = r#"
        struct Pair {
            a: i32,
            b: i32,
        }
        fn take_box(b: Box<Pair>) -> Box<Pair> {
            return b;
        }
        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "Box<Pair> should compile. Diags: {:?}",
        result.unwrap_err()
    );
}

// =============================================================================
// Phase 3: Verify memory lifecycle primitives in MVIR
// =============================================================================

/// Verify that __mellis_alloc / __mellis_dealloc FFI declarations
/// are accepted in Luna source without errors.
#[test]
fn test_alloc_dealloc_ffi_declarations() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("alloc_ffi_decl");
    let main_path = temp.join("main.ln");
    let src = r#"
        export extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        export extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "__mellis_alloc/dealloc FFI decls should compile. Diags: {:?}",
        result.unwrap_err()
    );
}

/// Verify that a function calling extern alloc/dealloc FFI passes semantic analysis.
///
/// Fixed: extern function signatures are now typed in populate_signatures_pass2,
/// so concrete raw pointer types like *rw u8 are properly resolved during monomorphization.
#[test]
fn test_alloc_dealloc_roundtrip_compiles() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("alloc_roundtrip");
    let main_path = temp.join("main.ln");
    // Use a wrapper function that accepts pre-typed u64 args
    // to avoid integer literal inference barriers with extern FFI
    let src = r#"
        export extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        export extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        fn do_alloc(size: u64, align: u64) -> *rw u8 {
            return __mellis_alloc(size, align);
        }

        fn do_dealloc(ptr: *rw u8, size: u64, align: u64) {
            __mellis_dealloc(ptr, size, align);
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check_semantic_only(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "Alloc + dealloc roundtrip should compile. Diags: {:?}",
        result.unwrap_err()
    );
}

/// Regression test: concrete raw pointer types (*rw u8, *u32) must pass
/// monomorphization barrier. Previously, extern function signatures weren't typed
/// in populate_signatures_pass2, causing unconstrained inference vars in pointer
/// pointee positions, which triggered false MonomorphizationBarrier errors.
#[test]
fn test_concrete_raw_pointer_types_monomorphic() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("concrete_ptr_mono");
    let main_path = temp.join("main.ln");
    let src = r#"
        // Extern functions with concrete raw pointer types
        export extern fn alloc_bytes(size: u64) -> *rw u8;
        export extern fn get_first(arr: *u32, idx: u32) -> u32;
        export extern fn store_value(ptr: *rw f64, val: f64);

        // Regular functions using concrete raw pointers
        fn process(ptr: *rw u8) -> i32 {
            return 0;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check_semantic_only(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "Concrete raw pointer types should be monomorphic. Diags: {:?}",
        result.unwrap_err()
    );
}

/// Verify that concrete Reference types also pass monomorphization barrier.
#[test]
fn test_concrete_reference_types_monomorphic() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("concrete_ref_mono");
    let main_path = temp.join("main.ln");
    let src = r#"
        fn get_ref(data: &i32) -> &i32 {
            return data;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check_semantic_only(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "Concrete reference types should be monomorphic. Diags: {:?}",
        result.unwrap_err()
    );
}

/// Verify sizeof/alignof intrinsics compile when used with concrete types.
#[test]
fn test_sizeof_alignof_intrinsics() {
    let (_sysroot, options) = test_sysroot_options();
    let temp = create_temp_dir("sizeof_alignof");
    let main_path = temp.join("main.ln");
    let src = r#"
        fn main() -> i32 {
            dec s = sizeof(i32);
            dec a = alignof(i32);
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let result = check(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        result.is_ok(),
        "sizeof/alignof should compile. Diags: {:?}",
        result.unwrap_err()
    );
}
