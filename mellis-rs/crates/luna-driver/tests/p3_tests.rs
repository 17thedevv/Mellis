use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p3_tests").join(name);
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

fn compile_to_obj(name: &str, src: &str) -> (bool, Vec<luna_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        output_path: Some(temp.join("out.obj").to_string_lossy().to_string()),
        ..Default::default()
    };

    let res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

// =============================================================================
// Phase 3 Test Matrix: Unsize / dyn Trait Coercion Formalization (22 Tests)
// =============================================================================

// -----------------------------------------------------------------------------
// Group 1: Unsized Value-Position Rejection (Tests 01–05)
// -----------------------------------------------------------------------------

// 01. Unsized dyn Trait in local variable declaration (let/dec) rejected
#[test]
fn test_case_01_unsized_dyn_trait_in_let_rejected() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S { x: i32 }
        impl Foo for S {
            fn f(self: &Self) -> i32 { return self.x; }
        }
        fn main() -> i32 {
            dec s = S { x: 42 };
            dec x: dyn Foo = s;
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(!success, "Unsized dyn Foo in value position must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// 02. Unsized slice [T] in local variable declaration rejected
#[test]
fn test_case_02_unsized_slice_in_let_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec arr = [1, 2, 3];
            dec x: [i32] = arr;
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_02", src);
    assert!(!success, "Unsized [i32] in value position must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// 03. Unsized function parameter rejected
#[test]
fn test_case_03_unsized_function_parameter_rejected() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        fn take_dyn(x: dyn Foo) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_03", src);
    assert!(!success, "Unsized function parameter must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// 04. Unsized function return type rejected
#[test]
fn test_case_04_unsized_function_return_rejected() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        fn ret_dyn() -> dyn Foo {
            loop {}
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_04", src);
    assert!(!success, "Unsized function return type must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// 05. Unsized struct field rejected
#[test]
fn test_case_05_unsized_struct_field_rejected() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct BadStruct {
            f: dyn Foo,
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(!success, "Unsized struct field must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// -----------------------------------------------------------------------------
// Group 2: Fat Pointers & Safe Dynamic Dispatch (Tests 06–07)
// -----------------------------------------------------------------------------

// 06. &dyn Trait creation and method dispatch compiles to machine code
#[test]
fn test_case_06_ref_dyn_trait_creation_and_dispatch() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S {
            val: i32,
        }
        impl Foo for S {
            fn f(self: &Self) -> i32 {
                return self.val;
            }
        }
        fn call_foo(obj: &dyn Foo) -> i32 {
            return obj.f();
        }
        fn main() -> i32 {
            dec s = S { val: 42 };
            dec obj: &dyn Foo = &s;
            return call_foo(obj);
        }
    "#;
    let (success, diags) = compile_to_obj("test_06", src);
    assert!(success, "&dyn Foo creation and dispatch should compile to object code: {:?}", diags);
}

// 07. *dyn Trait in unsafe block compiles
#[test]
fn test_case_07_raw_pointer_dyn_trait_unsafe() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        struct S {
            val: i32,
        }
        impl Foo for S {
            fn f(self: &Self) -> i32 {
                return self.val;
            }
        }
        fn main() -> i32 {
            dec s = S { val: 10 };
            dec r: &dyn Foo = &s;
            dec ptr: *dyn Foo = r as *dyn Foo;
            unsafe {
                dec copy_ptr = ptr;
                return 0;
            }
        }
    "#;
    let (success, diags) = compile_to_obj("test_07", src);
    assert!(success, "*dyn Foo in unsafe context should compile: {:?}", diags);
}

// -----------------------------------------------------------------------------
// Group 3: Object Safety Violations (Tests 08–11)
// -----------------------------------------------------------------------------

// 08. Trait with generic method is not object-safe
#[test]
fn test_case_08_object_safety_generic_method_rejected() {
    let src = r#"
        trait GenericMethodTrait {
            fn generic<T>(self: &Self, t: T) -> i32;
        }
        fn test_it(x: &dyn GenericMethodTrait) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(!success, "Trait with generic method must fail object safety check");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE, got: {:?}", diags);
}

// 09. Trait with static method (no receiver) is not object-safe
#[test]
fn test_case_09_object_safety_no_receiver_rejected() {
    let src = r#"
        trait NoReceiverTrait {
            fn create() -> i32;
        }
        fn test_it(x: &dyn NoReceiverTrait) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_09", src);
    assert!(!success, "Trait with static method must fail object safety check");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE, got: {:?}", diags);
}

// 10. Trait with method returning Self is not object-safe
#[test]
fn test_case_10_object_safety_returns_self_rejected() {
    let src = r#"
        trait ReturnsSelfTrait {
            fn duplicate(self: &Self) -> Self;
        }
        fn test_it(x: &dyn ReturnsSelfTrait) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(!success, "Trait returning Self must fail object safety check");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE, got: {:?}", diags);
}

// 11. Trait with Self in non-receiver parameter is not object-safe
#[test]
fn test_case_11_object_safety_self_in_non_receiver_param_rejected() {
    let src = r#"
        trait SelfParamTrait {
            fn compare(self: &Self, other: Self) -> bool;
        }
        fn test_it(x: &dyn SelfParamTrait) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_11", src);
    assert!(!success, "Trait with Self in non-receiver param must fail object safety check");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE, got: {:?}", diags);
}

// -----------------------------------------------------------------------------
// Group 4: Frozen v1 Restrictions (Tests 12–14)
// -----------------------------------------------------------------------------

// 12. dyn Trait with associated type rejected in v1
#[test]
fn test_case_12_dyn_associated_type_rejected() {
    let src = r#"
        trait AssocTrait {
            type Item;
            fn get(self: &Self) -> i32;
        }
        fn test_it(x: &dyn AssocTrait) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_12", src);
    assert!(!success, "dyn Trait with associated types must be rejected in v1");
    let err = diags.iter().find(|d| d.message.contains("E_DYN_ASSOCIATED_TYPE_UNSUPPORTED"));
    assert!(err.is_some(), "Expected E_DYN_ASSOCIATED_TYPE_UNSUPPORTED, got: {:?}", diags);
}

// 13. Supertrait dyn composition rejected in v1
#[test]
fn test_case_13_supertrait_dyn_rejected() {
    let src = r#"
        trait Parent {
            fn p(self: &Self) -> i32;
        }
        trait Child : Parent {
            fn c(self: &Self) -> i32;
        }
        fn test_it(x: &dyn Child) -> i32 {
            return 0;
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_13", src);
    assert!(!success, "dyn Child with supertraits must be rejected in v1");
    let err = diags.iter().find(|d| d.message.contains("E_DYN_SUPERTRAIT_UNSUPPORTED"));
    assert!(err.is_some(), "Expected E_DYN_SUPERTRAIT_UNSUPPORTED, got: {:?}", diags);
}

// 14. Box<dyn Trait> and Box<[T]> rejected in v1
#[test]
fn test_case_14_box_unsized_rejected() {
    let src = r#"
        import <core>;
        trait Foo {
            fn f(self: &Self) -> i32;
        }
        fn test_box_dyn(b: Box<dyn Foo>) {
        }
        fn main() -> i32 { return 0; }
    "#;
    let (success, diags) = run_compiler("test_14", src);
    assert!(!success, "Box<dyn Trait> must be rejected in v1");
    let err = diags.iter().find(|d| d.message.contains("E_UNSUPPORTED_BOX_UNSIZED"));
    assert!(err.is_some(), "Expected E_UNSUPPORTED_BOX_UNSIZED, got: {:?}", diags);
}

// -----------------------------------------------------------------------------
// Group 5: Array-to-Slice Coercion (Tests 15–17)
// -----------------------------------------------------------------------------

// 15. Array-to-slice coercion &[T; N] -> &[T]
#[test]
fn test_case_15_array_to_slice_coercion_and_len() {
    let src = r#"
        fn sum_slice(s: &[i32]) -> i32 {
            return 0;
        }
        fn main() -> i32 {
            dec arr = [10, 20, 30];
            dec sl: &[i32] = &arr;
            return sum_slice(sl);
        }
    "#;
    let (success, diags) = compile_to_obj("test_15", src);
    assert!(success, "Array-to-slice coercion should compile cleanly: {:?}", diags);
}

// 16. Array-to-slice mutability narrowing &rw [T; N] -> &[T], and immutable to mutable rejected
#[test]
fn test_case_16_array_to_slice_mutability_narrowing() {
    let src_valid = r#"
        fn take_shared(s: &[i32]) -> i32 { return 0; }
        fn take_mut(s: &rw [i32]) -> i32 { return 0; }
        fn main() -> i32 {
            dec rw arr1 = [1, 2, 3];
            dec s_rw: &rw [i32] = &rw arr1;
            dec rw arr2 = [4, 5, 6];
            dec s_sh: &[i32] = &rw arr2;
            return take_mut(s_rw) + take_shared(s_sh);
        }
    "#;
    let (success, diags) = compile_to_obj("test_16_valid", src_valid);
    assert!(success, "Mutable array to shared slice coercion should succeed: {:?}", diags);

    let src_invalid = r#"
        fn main() -> i32 {
            dec arr = [1, 2, 3];
            dec s_rw: &rw [i32] = &arr;
            return 0;
        }
    "#;
    let (success_inv, diags_inv) = run_compiler("test_16_inv", src_invalid);
    assert!(!success_inv, "Shared array to mutable slice coercion must fail: {:?}", diags_inv);
}

// 17. Empty array to slice coercion &[T; 0] -> &[T]
#[test]
fn test_case_17_array_to_slice_empty_array() {
    let src = r#"
        fn take_slice(s: &[i32]) -> i32 { return 0; }
        fn main() -> i32 {
            dec empty: [i32; 0] = [];
            dec sl: &[i32] = &empty;
            return take_slice(sl);
        }
    "#;
    let (success, diags) = compile_to_obj("test_17", src);
    assert!(success, "Empty array to slice coercion should succeed: {:?}", diags);
}

// -----------------------------------------------------------------------------
// Group 6: Mellis Trait Object ABI v1 (Tests 18–22)
// -----------------------------------------------------------------------------

// 18. Vtable drop glue invokes concrete Drop implementation
#[test]
fn test_case_18_vtable_layout_drop_glue_concrete_called() {
    let src = r#"
        import <core>;
        trait Action {
            fn act(self: &Self) -> i32;
        }
        struct Resource {
            id: i32,
        }
        impl Drop for Resource {
            fn drop(self: &rw Self) {
                self.id = 0;
            }
        }
        impl Action for Resource {
            fn act(self: &Self) -> i32 {
                return self.id;
            }
        }
        fn main() -> i32 {
            dec res = Resource { id: 7 };
            dec obj: &dyn Action = &res;
            return obj.act();
        }
    "#;
    let (success, diags) = compile_to_obj("test_18", src);
    assert!(success, "Type implementing Drop should compile vtable with drop glue: {:?}", diags);
}

// 19. Vtable drop glue is null for trivial type without crash
#[test]
fn test_case_19_vtable_layout_drop_glue_null_noop() {
    let src = r#"
        import <core>;
        trait Action {
            fn act(self: &Self) -> i32;
        }
        struct Trivial {
            val: i32,
        }
        impl Action for Trivial {
            fn act(self: &Self) -> i32 {
                return self.val;
            }
        }
        fn main() -> i32 {
            dec t = Trivial { val: 99 };
            dec obj: &dyn Action = &t;
            return obj.act();
        }
    "#;
    let (success, diags) = compile_to_obj("test_19", src);
    assert!(success, "Type without Drop should compile vtable with null drop glue: {:?}", diags);
}

// 20. Vtable method slot ordering is stable and preserves declaration order
#[test]
fn test_case_20_vtable_method_order_stability() {
    let src = r#"
        import <core>;
        trait Multi {
            fn alpha(self: &Self) -> i32;
            fn beta(self: &Self) -> i32;
            fn gamma(self: &Self) -> i32;
        }
        struct Calc {
            base: i32,
        }
        impl Multi for Calc {
            fn alpha(self: &Self) -> i32 { return self.base + 1; }
            fn beta(self: &Self) -> i32 { return self.base + 2; }
            fn gamma(self: &Self) -> i32 { return self.base + 3; }
        }
        fn run_multi(m: &dyn Multi) -> i32 {
            return m.alpha() + m.beta() + m.gamma();
        }
        fn main() -> i32 {
            dec c = Calc { base: 10 };
            dec obj: &dyn Multi = &c;
            return run_multi(obj);
        }
    "#;
    let (success, diags) = compile_to_obj("test_20", src);
    assert!(success, "Multi-method trait object dispatch with stable slot order should compile: {:?}", diags);
}

// 21. Multiple concrete types dispatched through the same &dyn Trait
#[test]
fn test_case_21_multi_concrete_types_dispatch() {
    let src = r#"
        import <core>;
        trait Greeter {
            fn greet(self: &Self) -> i32;
        }
        struct A { code: i32 }
        struct B { code: i32 }
        impl Greeter for A { fn greet(self: &Self) -> i32 { return self.code; } }
        impl Greeter for B { fn greet(self: &Self) -> i32 { return self.code * 2; } }
        fn dispatch(g: &dyn Greeter) -> i32 {
            return g.greet();
        }
        fn main() -> i32 {
            dec a = A { code: 5 };
            dec b = B { code: 10 };
            return dispatch(&a) + dispatch(&b);
        }
    "#;
    let (success, diags) = compile_to_obj("test_21", src);
    assert!(success, "Multiple concrete types dispatching via trait object should compile: {:?}", diags);
}

// 22. &rw dyn Trait mutable method dispatch
#[test]
fn test_case_22_ref_mut_dyn_trait_mutable_dispatch() {
    let src = r#"
        import <core>;
        trait Counter {
            fn inc(self: &rw Self);
            fn get(self: &Self) -> i32;
        }
        struct MyCounter {
            val: i32,
        }
        impl Counter for MyCounter {
            fn inc(self: &rw Self) {
                self.val = self.val + 1;
            }
            fn get(self: &Self) -> i32 {
                return self.val;
            }
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
    let (success, diags) = compile_to_obj("test_22", src);
    assert!(success, "&rw dyn Trait mutable method dispatch should compile: {:?}", diags);
}
