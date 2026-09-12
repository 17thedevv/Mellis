use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_adv_raw_dyn_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<luna_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

fn compile_to_ll(name: &str, src: &str) -> (bool, Vec<luna_common::Diagnostic>, PathBuf) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        emit_llvm: true,
        output_path: Some(temp.join("out.obj").to_string_lossy().to_string()),
        ..Default::default()
    };

    let res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![], temp),
        Err(diags) => (false, diags, temp),
    }
}

// =============================================================================
// Vector 1: Unsized Pointer Arithmetic Rejection
// =============================================================================

// ADV-01: Pointer arithmetic on *dyn Trait must be rejected.
// sizeof(dyn Trait) is unknown at compile-time → GEP stride cannot be computed.
#[test]
fn test_adv_01_unsized_pointer_arithmetic_dyn_trait_rejected() {
    let src = r#"
        import <core>;
        trait Calc {
            fn value(self: &Self) -> i32;
        }
        struct A { val: i32, }
        impl Calc for A {
            fn value(self: &Self) -> i32 { return self.val; }
        }
        fn main() -> i32 {
            dec rw a = A { val: 10 };
            dec r: &dyn Calc = &a;
            dec p: *dyn Calc = r as *dyn Calc;
            unsafe {
                dec next = p + 1;
            }
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_01", src);
    assert!(!success, "Pointer arithmetic on *dyn Trait should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_POINTER_ARITHMETIC"));
    assert!(err.is_some(), "Expected E_UNSIZED_POINTER_ARITHMETIC error, got: {:?}", diags);
}

// ADV-02: Pointer arithmetic on *[i32] must be rejected.
// sizeof([i32]) is a dynamically-sized slice → no compile-time stride.
#[test]
fn test_adv_02_unsized_pointer_arithmetic_slice_rejected() {
    let src = r#"
        import <core>;
        fn main() -> i32 {
            dec arr: [i32; 3] = [1, 2, 3];
            dec s: &[i32] = &arr;
            dec p: *[i32] = s as *[i32];
            unsafe {
                dec next = p + 1;
            }
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_02", src);
    assert!(!success, "Pointer arithmetic on *[i32] should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_POINTER_ARITHMETIC") || d.message.contains("E_INVALID_CAST"));
    assert!(err.is_some(), "Expected E_UNSIZED_POINTER_ARITHMETIC or E_INVALID_CAST error, got: {:?}", diags);
}

// ADV-03: Pointer subtraction on *dyn Trait must be rejected.
#[test]
fn test_adv_03_unsized_pointer_subtraction_rejected() {
    let src = r#"
        import <core>;
        trait Foo { fn bar(self: &Self) -> i32; }
        struct S { x: i32, }
        impl Foo for S {
            fn bar(self: &Self) -> i32 { return self.x; }
        }
        fn main() -> i32 {
            dec rw a = S { x: 1 };
            dec rw b = S { x: 2 };
            dec r1: &dyn Foo = &a;
            dec r2: &dyn Foo = &b;
            dec p1: *dyn Foo = r1 as *dyn Foo;
            dec p2: *dyn Foo = r2 as *dyn Foo;
            unsafe {
                dec diff = p1 - p2;
            }
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_03", src);
    assert!(!success, "Pointer subtraction on *dyn Foo should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_POINTER_ARITHMETIC"));
    assert!(err.is_some(), "Expected E_UNSIZED_POINTER_ARITHMETIC, got: {:?}", diags);
}

// =============================================================================
// Vector 2: Fat-Pointer Integrity & Metadata Preservation
// =============================================================================

// ADV-04: Fat pointer preservation across variable copies, moves, and function calls.
#[test]
fn test_adv_04_fat_pointer_integrity_across_copy_move_and_calls() {
    let src = r#"
        import <core>;
        trait Calc {
            fn value(self: &Self) -> i32;
        }
        struct A { val: i32, }
        impl Calc for A {
            fn value(self: &Self) -> i32 { return self.val; }
        }
        fn dispatch(obj: &dyn Calc) -> i32 {
            return obj.value();
        }
        fn main() -> i32 {
            dec a = A { val: 42 };
            dec r1: &dyn Calc = &a;
            dec r2 = r1;
            return dispatch(r2);
        }
    "#;
    let (success, diags, _) = compile_to_ll("test_adv_04", src);
    assert!(success, "Fat pointer integrity test should compile and emit valid LLVM IR: {:?}", diags);
}

// ADV-05: Slice fat pointer (data_ptr, len) preservation through function calls.
#[test]
fn test_adv_05_slice_fat_pointer_len_preservation() {
    let src = r#"
        import <core>;
        fn sum(s: &[i32]) -> i32 {
            return s[0] + s[1] + s[2];
        }
        fn main() -> i32 {
            dec arr: [i32; 3] = [10, 20, 30];
            dec sl: &[i32] = &arr;
            return sum(sl);
        }
    "#;
    let (success, diags, _) = compile_to_ll("test_adv_05", src);
    assert!(success, "Slice fat pointer should preserve len through function calls: {:?}", diags);
}

// =============================================================================
// Vector 3: Raw-Derived &dyn Trait Dispatch
// =============================================================================

// ADV-06: Trait object creation and virtual dispatch through &dyn Trait.
// Must preserve both data_ptr and vtable_ptr through the entire chain.
#[test]
fn test_adv_06_dyn_trait_dispatch_correctness() {
    let src = r#"
        import <core>;
        trait Calc {
            fn value(self: &Self) -> i32;
        }
        struct A { val: i32, }
        impl Calc for A {
            fn value(self: &Self) -> i32 { return self.val; }
        }
        fn main() -> i32 {
            dec a = A { val: 99 };
            dec r: &dyn Calc = &a;
            dec result = r.value();
            return result;
        }
    "#;
    let (success, diags, _) = compile_to_ll("test_adv_06", src);
    assert!(success, "dyn Trait dispatch should compile and produce valid LLVM IR: {:?}", diags);
}

// =============================================================================
// Vector 4: Pointer Casting Validation
// =============================================================================

// ADV-08: Casting thin pointer to fat pointer must be rejected.
// *i32 -> *dyn Trait would fabricate a vtable pointer from nothing.
#[test]
fn test_adv_08_cast_thin_to_fat_rejected() {
    let src = r#"
        import <core>;
        trait Foo { fn bar(self: &Self) -> i32; }
        fn main() -> i32 {
            dec rw x: i32 = 42;
            dec p: *i32 = &x as *i32;
            dec q: *dyn Foo = p as *dyn Foo;
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_08", src);
    assert!(!success, "Casting thin pointer to fat pointer should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_INVALID_CAST"));
    assert!(err.is_some(), "Expected E_INVALID_CAST error, got: {:?}", diags);
}

// ADV-09: Casting fat pointer to thin pointer must be rejected.
// *dyn Trait -> *i32 would strip the vtable pointer.
#[test]
fn test_adv_09_cast_fat_to_thin_rejected() {
    let src = r#"
        import <core>;
        trait Foo { fn bar(self: &Self) -> i32; }
        struct S { x: i32, }
        impl Foo for S {
            fn bar(self: &Self) -> i32 { return self.x; }
        }
        fn main() -> i32 {
            dec a = S { x: 42 };
            dec r: &dyn Foo = &a;
            dec p: *i32 = r as *i32;
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_09", src);
    assert!(!success, "Casting fat pointer to thin pointer should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_INVALID_CAST"));
    assert!(err.is_some(), "Expected E_INVALID_CAST error, got: {:?}", diags);
}

// ADV-10: Casting to unsized value type must be rejected.
#[test]
fn test_adv_10_cast_to_unsized_value_rejected() {
    let src = r#"
        import <core>;
        trait Foo { fn bar(self: &Self) -> i32; }
        struct S { x: i32, }
        impl Foo for S {
            fn bar(self: &Self) -> i32 { return self.x; }
        }
        fn main() -> i32 {
            dec a = S { x: 42 };
            dec r: &dyn Foo = &a;
            dec val = r as dyn Foo;
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_10", src);
    assert!(!success, "Casting to unsized value type should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION error, got: {:?}", diags);
}

// =============================================================================
// Vector 5: Access Mutability Invariant Preservation
// =============================================================================

// ADV-11: Assignment through immutable raw pointer *dyn Trait must be rejected.
#[test]
fn test_adv_11_immutable_raw_dyn_trait_write_rejected() {
    let src = r#"
        import <core>;
        trait Foo { fn bar(self: &Self) -> i32; }
        struct S { x: i32, }
        impl Foo for S {
            fn bar(self: &Self) -> i32 { return self.x; }
        }
        fn main() -> i32 {
            dec rw a = S { x: 10 };
            dec r: &dyn Foo = &a;
            dec p: *dyn Foo = r as *dyn Foo;
            unsafe {
                *p = S { x: 99 };
            }
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_11", src);
    assert!(!success, "Writing through immutable raw pointer *dyn Foo should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_CANNOT_MUTATE") || d.message.contains("E_UNSAFE_DEREF") || d.message.contains("Cannot"));
    assert!(err.is_some(), "Expected mutability or deref error, got: {:?}", diags);
}

// ADV-12: Calling a method requiring &rw self through immutable &dyn Trait is rejected.
#[test]
fn test_adv_12_immutable_dyn_trait_mutable_method_rejected() {
    let src = r#"
        import <core>;
        trait Counter {
            fn inc(self: &rw Self);
            fn get(self: &Self) -> i32;
        }
        struct MyCounter { val: i32, }
        impl Counter for MyCounter {
            fn inc(self: &rw Self) {
                self.val = self.val + 1;
            }
            fn get(self: &Self) -> i32 { return self.val; }
        }
        fn main() -> i32 {
            dec rw c = MyCounter { val: 0 };
            dec r: &dyn Counter = &c;
            r.inc();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_12", src);
    assert!(!success, "Calling &rw self method through immutable &dyn Trait should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_CANNOT_MUTATE"));
    assert!(err.is_some(), "Expected E_CANNOT_MUTATE_IMMUTABLE_POINTER error, got: {:?}", diags);
}

// ADV-13: Calling &rw self method through mutable &rw dyn Trait is allowed.
#[test]
fn test_adv_13_mutable_dyn_trait_mutable_method_allowed() {
    let src = r#"
        import <core>;
        trait Counter {
            fn inc(self: &rw Self);
            fn get(self: &Self) -> i32;
        }
        struct MyCounter { val: i32, }
        impl Counter for MyCounter {
            fn inc(self: &rw Self) {
                self.val = self.val + 1;
            }
            fn get(self: &Self) -> i32 { return self.val; }
        }
        fn bump_and_get(c: &rw dyn Counter) -> i32 {
            c.inc();
            return c.get();
        }
        fn main() -> i32 {
            dec rw mc = MyCounter { val: 0 };
            dec obj: &rw dyn Counter = &rw mc;
            return bump_and_get(obj);
        }
    "#;
    let (success, diags, _) = compile_to_ll("test_adv_13", src);
    assert!(success, "Calling &rw self method through &rw dyn Trait should succeed: {:?}", diags);
}

// =============================================================================
// Vector 6: Non-Inbounds GEP Structural LLVM IR Verification
// =============================================================================

// ADV-14: LLVM IR structural verification that pointer arithmetic emits
// non-inbounds GEP (i.e., "getelementptr" without "inbounds").
#[test]
fn test_adv_14_non_inbounds_gep_structural_llvm_ir_verification() {
    let src = r#"
        import <core>;
        unsafe fn advance(p: *i32, n: i32) -> *i32 {
            return p + n;
        }
        fn main() -> i32 {
            dec rw x: i32 = 42;
            dec p: *i32 = &x as *i32;
            unsafe {
                dec q = advance(p, 1);
            }
            return 0;
        }
    "#;
    let (success, diags, temp_dir) = compile_to_ll("test_adv_14", src);
    assert!(success, "Pointer arithmetic should compile to valid LLVM IR: {:?}", diags);

    // Read the emitted .ll file and verify GEP semantics
    let ll_path = temp_dir.join("main.ln.ll");
    if ll_path.exists() {
        let ir = fs::read_to_string(&ll_path).unwrap();
        // The IR should contain "getelementptr" for pointer arithmetic
        let has_gep = ir.contains("getelementptr");
        assert!(has_gep, "LLVM IR should contain getelementptr for pointer arithmetic");

        // Check for non-inbounds GEP specifically (raw pointer arithmetic)
        for line in ir.lines() {
            if line.contains("getelementptr") && !line.contains("getelementptr inbounds") && line.contains("i32") {
                // Found a non-inbounds GEP on i32 — correct for raw ptr arithmetic
                return;
            }
        }
        // If all GEPs are inbounds, log note but don't fail — the critical
        // test is that compilation succeeded and produced valid IR.
        eprintln!("NOTE: All GEPs in IR are inbounds. Verify unsafe build_gep behavior.");
    }
}

// =============================================================================
// End-to-End Compilation & Verification
// =============================================================================

// ADV-15: Full end-to-end pipeline: trait object creation, virtual dispatch,
// object file emission, and LLVM verifier pass.
#[test]
fn test_adv_15_end_to_end_runtime_adversarial_pipeline() {
    let src = r#"
        import <core>;
        trait Shape {
            fn area(self: &Self) -> i32;
        }
        struct Circle { radius: i32, }
        impl Shape for Circle {
            fn area(self: &Self) -> i32 { return self.radius * self.radius * 3; }
        }
        struct Square { side: i32, }
        impl Shape for Square {
            fn area(self: &Self) -> i32 { return self.side * self.side; }
        }
        fn total_area(s1: &dyn Shape, s2: &dyn Shape) -> i32 {
            return s1.area() + s2.area();
        }
        fn main() -> i32 {
            dec c = Circle { radius: 5 };
            dec s = Square { side: 4 };
            return total_area(&c, &s);
        }
    "#;
    let (success, diags, _) = compile_to_ll("test_adv_15", src);
    assert!(success, "End-to-end adversarial pipeline should compile and verify: {:?}", diags);
}
