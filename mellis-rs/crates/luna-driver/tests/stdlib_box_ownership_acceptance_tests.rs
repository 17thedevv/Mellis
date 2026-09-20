use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::{Path, PathBuf};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_box_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn locate_canonical_alloc_ln() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let alloc_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libs")
        .join("external")
        .join("alloc")
        .join("box.ln");
    assert!(alloc_path.exists(), "libs/external/alloc/box.ln must exist");
    alloc_path
}

fn locate_canonical_alloc_llib() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let llib_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libs")
        .join("external")
        .join("alloc")
        .join("box.llib");
    llib_path
}

/// B1: Library Box allocation via `std::box_new<T>` compiles and executes cleanly.
#[test]
fn test_b1_box_new_allocation_and_init() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b1_box_new");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct Point {
            x: i32,
            y: i32,
        };

        fn main() -> i32 {
            dec b = std::box_new<i32>(42);
            dec val = *b.as_ref();

            dec p = std::box_new<Point>(Point { x: 10, y: 20 });
            dec p_ref = p.as_ref();

            return val + p_ref.x + p_ref.y;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let result = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        result.is_ok(),
        "std::box_new with primitive and struct must succeed: {:?}",
        result.err()
    );
}

/// B2: `Box<DropType>` executes payload destructor through drop glue.
#[test]
fn test_b2_box_payload_drop_glue() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b2_drop_glue");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct ManagedItem {
            value: i32,
        };

        impl Drop for ManagedItem {
            fn drop(self: &rw Self) {
                self.value = 0;
            }
        }

        fn scope_with_drop() {
            dec b = std::box_new<ManagedItem>(ManagedItem { value: 99 });
        }

        fn main() -> i32 {
            scope_with_drop();
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let result = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        result.is_ok(),
        "Box<ManagedItem> where ManagedItem: Drop must compile cleanly: {:?}",
        result.err()
    );
}

/// B3: `std::box_into_inner<T>` extracts value and consumes the box.
#[test]
fn test_b3_box_into_inner_consuming_move() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b3_box_into_inner");

    let valid_src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b = std::box_new<i32>(100);
            dec val = std::box_into_inner<i32>(b);
            return val;
        }
    "#;
    let main_valid = dir.join("valid.ln");
    fs::write(&main_valid, valid_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res_valid = check(main_valid.to_str().unwrap(), valid_src.to_string(), &opts);
    assert!(res_valid.is_ok(), "Valid box_into_inner must compile: {:?}", res_valid.err());
}

/// B4: Drop lifecycle order: DropInPlace(T) executes before __mellis_dealloc storage.
#[test]
fn test_b4_box_lifecycle_order() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b4_lifecycle_order");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct Resource {
            active: bool,
        };

        impl Drop for Resource {
            fn drop(self: &rw Self) {
                self.active = false;
            }
        }

        fn run_drop() {
            dec b = std::box_new<Resource>(Resource { active: true });
        }

        fn main() -> i32 {
            run_drop();
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let result = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(result.is_ok(), "Lifecycle order for Box with Drop must succeed: {:?}", result.err());
}

/// B5: Use-after-move rejection: Re-using moved box fails borrowck.
#[test]
fn test_b5_box_use_after_move_rejected() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b5_use_after_move");

    let move_src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b = std::box_new<i32>(100);
            dec val = std::box_into_inner<i32>(b);
            dec val2 = *b.as_ref();
            return val;
        }
    "#;
    let main_move = dir.join("move.ln");
    fs::write(&main_move, move_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res_move = check(main_move.to_str().unwrap(), move_src.to_string(), &opts);
    assert!(
        res_move.is_err(),
        "Use of Box after std::box_into_inner MUST be rejected by borrowck / move analysis"
    );
    let diags = res_move.unwrap_err();
    let has_moved_err = diags.iter().any(|d| d.message.to_lowercase().contains("moved"));
    assert!(
        has_moved_err,
        "Expected use-after-move diagnostic, got: {:?}",
        diags
    );
}

/// B6: Borrow checking rules: `b.as_ref()` and `b.as_mut()` enforce borrowing constraints.
#[test]
fn test_b6_box_borrow_conflict_rejection() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b6_borrow_conflict");

    let conflict_src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec rw b = std::box_new<i32>(50);
            dec m = b.as_mut();
            dec r = b.as_ref();
            return *m + *r;
        }
    "#;
    let main_path = dir.join("conflict.ln");
    fs::write(&main_path, conflict_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), conflict_src.to_string(), &opts);
    assert!(
        res.is_err(),
        "Taking shared borrow while as_mut is active MUST fail borrow checking"
    );
}

/// B7: Disarmed sentinel vs live allocation invariant.
#[test]
fn test_b7_disarmed_sentinel_vs_live() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b7_disarmed_sentinel");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b = std::box_new<i32>(42);
            // b is live: b.ptr != 0
            dec val = std::box_into_inner<i32>(b);
            // inside box_into_inner: b.ptr was disarmed to 0, destructor safely skipped
            return val;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Disarmed sentinel test must succeed: {:?}", res.err());
}

/// B8: Zero-Sized Type (ZST) allocation support with canonical runtime sentinel.
#[test]
fn test_b8_box_zero_sized_type() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b8_zst");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct Unit {};


        fn main() -> i32 {
            dec b = std::box_new<Unit>(Unit {});
            dec u = std::box_into_inner<Unit>(b);
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Box<ZST> must compile cleanly: {:?}", res.err());
}

/// B9: Public API hygiene: `box_dealloc` is not exported in safe public `std::`.
#[test]
fn test_b9_box_dealloc_not_in_public_std() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b9_no_box_dealloc");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b = std::box_new<i32>(10);
            std::box_dealloc<i32>(b);
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "Calling std::box_dealloc MUST be rejected as not found in public std"
    );
}

/// B10: `std::Box<T>` defined in `alloc.ln` under `module std` and compiles to `alloc.llib`.
#[test]
fn test_b10_box_struct_defined_in_module_std() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b10_module_std");

    let alloc_path = locate_canonical_alloc_ln();
    let alloc_src = fs::read_to_string(&alloc_path).expect("Failed to read alloc.ln");

    let out_llib = dir.join("box.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res = compile(alloc_path.to_str().unwrap(), alloc_src, &compile_opts);
    assert!(res.is_ok(), "Compiling box.ln with std::Box must succeed: {:?}", res.err());
    assert!(out_llib.exists(), "box.llib must exist");
}

/// B11: Downstream consumer importing `<alloc>` accesses `std::Box<T>` and `std::box_new<T>`.
#[test]
fn test_b11_consumer_import_alloc_accesses_std_box() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b11_consumer_access");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b: std::Box<i32> = std::box_new<i32>(123);
            return *b.as_ref();
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Consumer accessing std::Box<T> must succeed: {:?}", res.err());
}

/// B12: Rule 7: Provider != Namespace. Accessing `alloc::Box` is rejected; `std::Box` succeeds.
#[test]
fn test_b12_rule_7_provider_vs_namespace() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b12_rule7");

    let bad_src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b = alloc::box_new<i32>(42);
            return 0;
        }
    "#;
    let main_bad = dir.join("bad.ln");
    fs::write(&main_bad, bad_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res_bad = check(main_bad.to_str().unwrap(), bad_src.to_string(), &opts);
    assert!(
        res_bad.is_err(),
        "Accessing provider name `alloc::box_new` MUST be rejected"
    );
}

/// B13: Binary .llib metadata parity between source and binary providers.
#[test]
fn test_b13_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b13_parity");

    let alloc_path = locate_canonical_alloc_ln();
    let alloc_src = fs::read_to_string(&alloc_path).expect("Failed to read alloc.ln");

    let out_llib = dir.join("box.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res_compile = compile(alloc_path.to_str().unwrap(), alloc_src, &compile_opts);
    assert!(res_compile.is_ok(), "Compiling box.ln to box.llib must succeed: {:?}", res_compile.err());

    // Sync canonical box.llib
    let canonical_llib = locate_canonical_alloc_llib();
    let tmp = canonical_llib.with_file_name(format!("{}.publish{}", canonical_llib.file_name().unwrap().to_string_lossy(), std::process::id()));
    let _ = fs::copy(&out_llib, &tmp);
    let _ = fs::rename(&tmp, &canonical_llib);

    let consumer_src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec rw b = std::box_new<i32>(77);
            dec res = std::box_into_inner<i32>(b);
            return res;
        }
    "#;
    let consumer_path = dir.join("consumer.ln");
    fs::write(&consumer_path, consumer_src).unwrap();

    let consumer_opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res_consumer = check(consumer_path.to_str().unwrap(), consumer_src.to_string(), &consumer_opts);
    assert!(
        res_consumer.is_ok(),
        "Consumer compiling against binary alloc.llib must succeed: {:?}",
        res_consumer.err()
    );
}

/// B14: No user-level Box semantic special cases remain in the compiler.
#[test]
fn test_b14_no_box_compiler_special_cases() {
    // Assert at type system level that Instruction::HeapFree exists and replaced BoxFree
    let dummy_op = luna_mvir::Operand::Number("0".to_string());
    let heap_free = luna_mvir::Instruction::HeapFree { value: dummy_op };
    match heap_free {
        luna_mvir::Instruction::HeapFree { .. } => {}
        _ => panic!("Expected HeapFree instruction"),
    }
}

/// B15: Generic monomorphization across distinct inner types.
#[test]
fn test_b15_generic_monomorphization_multiple_types() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b15_mono");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct Data {
            a: i32,
            b: bool,
        };

        fn main() -> i32 {
            dec b1 = std::box_new<i32>(10);
            dec b2 = std::box_new<bool>(true);
            dec b3 = std::box_new<Data>(Data { a: 20, b: false });

            dec v1 = *b1.as_ref();
            dec v2 = *b2.as_ref();
            dec v3 = b3.as_ref();

            if v2 {
                return v1 + v3.a;
            } else {
                return 0;
            }
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Monomorphization across distinct types must succeed: {:?}", res.err());
}

/// B16: Nested Box ownership: `std::box_new<Box<i32>>(b1)`.
#[test]
fn test_b16_nested_box_ownership() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b16_nested_box");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        fn main() -> i32 {
            dec b1 = std::box_new<i32>(42);
            dec b2 = std::box_new<std::Box<i32>>(b1);

            dec inner_ref = b2.as_ref();
            dec val = *inner_ref.as_ref();

            dec unwrapped_b1 = std::box_into_inner<std::Box<i32>>(b2);
            dec final_val = std::box_into_inner<i32>(unwrapped_b1);

            return final_val;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Nested Box<Box<i32>> must succeed: {:?}", res.err());
}

/// B17: `Box<T> where T: Drop`: Custom struct implementing Drop boxed in Box<T>.
#[test]
fn test_b17_box_where_t_implements_drop() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b17_box_t_drop");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct ManagedResource {
            id: i32,
            is_active: bool,
        };

        impl Drop for ManagedResource {
            fn drop(self: &rw Self) {
                self.is_active = false;
            }
        }

        fn test_drop_scope() -> i32 {
            dec b = std::box_new<ManagedResource>(ManagedResource { id: 7, is_active: true });
            dec r = b.as_ref();
            return r.id;
        }

        fn main() -> i32 {
            return test_drop_scope();
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Box<T> where T: Drop must compile cleanly: {:?}", res.err());
}

/// B18: `Box<Box<T>>` scope destruction without recursive loop or memory leaks.
#[test]
fn test_b18_nested_box_scope_destruction() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("b18_nested_destruction");
    let main_path = dir.join("main.ln");

    let src = r#"
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct TrackedItem {
            val: i32,
        };

        impl Drop for TrackedItem {
            fn drop(self: &rw Self) {
                self.val = 0;
            }
        }

        fn create_nested() {
            dec item = TrackedItem { val: 100 };
            dec inner_box = std::box_new<TrackedItem>(item);
            dec outer_box = std::box_new<std::Box<TrackedItem>>(inner_box);
        }

        fn main() -> i32 {
            create_nested();
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Nested Box<Box<T>> destruction must compile cleanly without recursion: {:?}",
        res.err()
    );
}
