use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p0b_tests").join(name);
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

// 1. Whole place move (Allowed)
#[test]
fn test_p0b_whole_place_move() {
    let src = r#"
        import <core>;
        struct HasDrop { val: i32 }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec x = HasDrop { val: 42 };
            dec y = x; // Valid whole-place move
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_whole_place_move", src);
    assert!(success, "Compilation should succeed for valid whole place move. Found diagnostics: {:?}", diagnostics);
}

// 2. Subplace borrow (Allowed)
#[test]
fn test_p0b_subplace_borrow() {
    let src = r#"
        import <core>;
        struct HasDrop { val: i32 }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec rw x = HasDrop { val: 42 };
            dec ptr = &rw x.val; // Valid subplace borrow
            *ptr = 100;
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_subplace_borrow", src);
    assert!(success, "Compilation should succeed for subplace borrow. Found diagnostics: {:?}", diagnostics);
}

// 3. Proper subplace move: c.field (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_field() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct HasDrop { val: Inner }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec x = HasDrop { val: Inner { v: 42 } };
            dec sub = x.val; // Error! Move out of subplace of Drop type
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_field", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 4. Proper subplace move: c.nested.field (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_nested_field() {
    let src = r#"
        import <core>;
        struct Bottom { v: i32 }
        struct Middle { b: Bottom }
        struct TopDrop { m: Middle }
        impl Drop for TopDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec t = TopDrop { m: Middle { b: Bottom { v: 42 } } };
            dec b_val = t.m.b; // Error!
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_nested_field", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 5. Proper subplace move: Tuple field (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_tuple_field() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct TupleDrop { t: (Inner, i32) }
        impl Drop for TupleDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec x = TupleDrop { t: (Inner { v: 42 }, 10) };
            dec inner = x.t.0; // Error! Moves a proper subplace out of a type that implements Drop!
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_tuple_field", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 6. Enum payload pattern matching (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_enum_payload() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        enum DropEnum {
            A(Inner),
            B
        }
        impl Drop for DropEnum {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec e = DropEnum::A(Inner { v: 42 });
            match e {
                DropEnum::A(val) -> { return 1; }, // Error! Moves payload out of DropEnum
                DropEnum::B -> { return 2; }
            };
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_enum_payload", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 7. Pattern matching by-value (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_pattern_matching() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct HasDrop { val: Inner }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec e = DropEnum::A(Inner { v: 42 });
            dec DropEnum::A(val) = e;
            return 0;
        }
        enum DropEnum {
            A(Inner),
            B
        }
        impl Drop for DropEnum {
            fn drop(self: &rw Self) {}
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_pattern_matching", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 8. Function argument by-value (Forbidden)
#[test]
fn test_p0b_proper_subplace_move_function_argument() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct HasDrop { val: Inner }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn consume(i: Inner) {}
        fn main() -> i32 {
            dec x = HasDrop { val: Inner { v: 42 } };
            consume(x.val); // Error!
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_proper_subplace_move_function_argument", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 9. Generic instantiated Drop type (Forbidden)
#[test]
fn test_p0b_generic_instantiated_drop_type() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct Wrapper<T> { val: T }
        impl<T> Drop for Wrapper<T> {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec w = Wrapper<Inner> { val: Inner { v: 42 } };
            dec inner = w.val; // Error! w is instantiated and implements Drop
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_generic_instantiated_drop_type", src);
    let has_error = diagnostics.iter().any(|d| d.message.contains("E_PARTIAL_MOVE_UNDER_DROP"));
    assert!(has_error, "Expected E_PARTIAL_MOVE_UNDER_DROP error, found: {:?}", diagnostics);
}

// 10. Field reassignment (Allowed)
#[test]
fn test_p0b_field_reassignment() {
    let src = r#"
        import <core>;
        struct Inner { v: i32 }
        struct HasDrop { val: Inner }
        impl Drop for HasDrop {
            fn drop(self: &rw Self) {}
        }
        fn main() -> i32 {
            dec rw x = HasDrop { val: Inner { v: 42 } };
            x.val = Inner { v: 100 }; // Valid field reassignment
            return 0;
        }
    "#;
    let (success, diagnostics) = run_compiler("test_p0b_field_reassignment", src);
    assert!(success, "Compilation should succeed for field reassignment. Found diagnostics: {:?}", diagnostics);
}
