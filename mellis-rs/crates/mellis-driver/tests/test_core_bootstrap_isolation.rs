use mellis_driver::sysroot::Sysroot;
use mellis_driver::{check, CompilerOptions};
use mellis_common::CompilerSession;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_core_isolation_tests").join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

/// Invariant C: Core symbols do NOT appear in user namespace without `import <core>;`
#[test]
fn test_invariant_c_no_implicit_core_symbols_without_import() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("invariant_c");
    let main_path = dir.join("main.ms");

    // 1. Check Result is unresolved without import
    let src_result = r#"
        fn foo(x: Result<i32, string>) {}
    "#;
    fs::write(&main_path, src_result).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src_result.to_string(), &opts);
    assert!(res.is_err(), "Result must NOT be visible without import");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("cannot find type `Result`") || d.message.contains("unresolved") || d.message.contains("not found")),
        "Expected unresolved error for Result, got: {:?}", errs
    );

    // 2. Check Drop trait is unresolved without import
    let src_drop = r#"
        fn bar(x: Drop) {}
    "#;
    fs::write(&main_path, src_drop).unwrap();
    let res = check(main_path.to_str().unwrap(), src_drop.to_string(), &opts);
    assert!(res.is_err(), "Drop must NOT be visible without import");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("cannot find type `Drop`") || d.message.contains("unresolved") || d.message.contains("not found")),
        "Expected unresolved error for Drop, got: {:?}", errs
    );

    // 3. Check ControlFlow is unresolved without import
    let src_cf = r#"
        fn baz(x: ControlFlow<i32, string>) {}
    "#;
    fs::write(&main_path, src_cf).unwrap();
    let res = check(main_path.to_str().unwrap(), src_cf.to_string(), &opts);
    assert!(res.is_err(), "ControlFlow must NOT be visible without import");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("cannot find type `ControlFlow`") || d.message.contains("unresolved") || d.message.contains("not found")),
        "Expected unresolved error for ControlFlow, got: {:?}", errs
    );
}

/// Invariant D: Explicitly imported core API works normally via `core::...`
#[test]
fn test_invariant_d_explicit_import_core_works() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("invariant_d");
    let main_path = dir.join("main.ms");

    let src = r#"
        import <core>;

        fn make_result() -> core::Result<i32, i32> {
            return core::Result::Ok(42);
        }

        fn make_cf() -> core::ControlFlow<i32, str> {
            return core::ControlFlow::Continue("hello");
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Importing core and accessing core::Result, core::ControlFlow must succeed, got: {:?}", res.err());
}

/// Invariant A: `?` operator does NOT require lexical visibility of `Try` or `FromResidual`
#[test]
fn test_invariant_a_try_operator_independent_of_lexical_try() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("invariant_a");
    let main_path = dir.join("main.ms");

    let src = r#"
        import <core>;

        fn step1() -> core::Result<i32, i32> {
            return core::Result::Ok(10);
        }

        fn step2() -> core::Result<i32, i32> {
            dec val = step1()?;
            return core::Result::Ok(val + 5);
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "`?` operator desugaring must succeed via LangItem::Try without Try in lexical scope: {:?}", res.err());

    // Also assert that `Try` itself is NOT in lexical scope
    let src_bad = r#"
        import <core>;

        fn check_try_not_in_scope(x: Try) {}
    "#;
    fs::write(&main_path, src_bad).unwrap();
    let res_bad = check(main_path.to_str().unwrap(), src_bad.to_string(), &opts);
    assert!(res_bad.is_err(), "Trait `Try` must NOT be in lexical scope even with `import <core>;` unless accessed via `core::Try`");
}

/// Invariant B: `Drop` semantics do NOT require lexical visibility of `Drop`
#[test]
fn test_invariant_b_drop_independent_of_lexical_drop() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("invariant_b");
    let main_path = dir.join("main.ms");

    let src = r#"
        import <core>;

        struct Handle {
            id: i32,
        }

        impl core::Drop for Handle {
            fn drop(self: &rw Handle) {
                // cleanup
            }
        }

        fn process() {
            dec h = Handle { id: 1 };
            // Drop should be checked and elaborated without `Drop` in lexical scope
        }

        fn main() -> i32 {
            process();
            return 0;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Implementing core::Drop must succeed without bare Drop in lexical scope: {:?}", res.err());
}

/// Invariant E: LangItem registry points to canonical declaration identities from `core.ms`
#[test]
fn test_invariant_e_lang_items_mapped_to_canonical_declarations() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let mut compiler_session = CompilerSession::new();
    let mut driver_session = mellis_driver::session::DriverSession::new(sysroot, &mut compiler_session, &[]);

    let mut arena = mellis_ast::AstArena::new();
    let mut source = String::new();

    let core_id = driver_session.bootstrap_core(&mut arena)
        .expect("Failed to bootstrap core");

    let interface = driver_session.registry.interfaces.get(&core_id)
        .expect("Core interface must exist in registry");

    // Verify all 6 frozen lang items are registered in core interface
    use mellis_semantic::lang_item::LangItem;
    assert!(interface.lang_items.contains_key(&LangItem::Drop), "Drop must be registered");
    assert!(interface.lang_items.contains_key(&LangItem::FromResidual), "FromResidual must be registered");
    assert!(interface.lang_items.contains_key(&LangItem::Try), "Try must be registered");
    assert!(interface.lang_items.contains_key(&LangItem::ControlFlow), "ControlFlow must be registered");
    assert!(interface.lang_items.contains_key(&LangItem::ControlFlowContinue), "Continue must be registered");
    assert!(interface.lang_items.contains_key(&LangItem::ControlFlowBreak), "Break must be registered");

    // Verify when injected into SemanticContext, LangItemRegistry is fully populated
    let mut semantic_ctx = mellis_semantic::SemanticContext::new();
    driver_session.registry.inject_into_ctx(&mut semantic_ctx);

    assert!(semantic_ctx.lang_items.get(LangItem::Drop).is_some());
    assert!(semantic_ctx.lang_items.get(LangItem::FromResidual).is_some());
    assert!(semantic_ctx.lang_items.get(LangItem::Try).is_some());
    assert!(semantic_ctx.lang_items.get(LangItem::ControlFlow).is_some());
    assert!(semantic_ctx.lang_items.get(LangItem::ControlFlowContinue).is_some());
    assert!(semantic_ctx.lang_items.get(LangItem::ControlFlowBreak).is_some());

    // Verify that ScopeId(0) does NOT contain core symbols
    let global_scope = mellis_semantic::symbol::ScopeId(0);
    assert!(semantic_ctx.symbol_table.lookup("Drop", global_scope).is_none(), "Drop must NOT be in global scope");
    assert!(semantic_ctx.symbol_table.lookup("Result", global_scope).is_none(), "Result must NOT be in global scope");
    assert!(semantic_ctx.symbol_table.lookup("Try", global_scope).is_none(), "Try must NOT be in global scope");
    assert!(semantic_ctx.symbol_table.lookup("ControlFlow", global_scope).is_none(), "ControlFlow must NOT be in global scope");
}
