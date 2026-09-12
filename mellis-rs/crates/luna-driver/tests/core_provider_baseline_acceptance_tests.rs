use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use luna_llib::MlibReader;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("mellis_core_baseline_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn locate_canonical_core_ln() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let core_ln = manifest_dir.join("../../libs/external/core.ln");
    if core_ln.exists() {
        return core_ln.canonicalize().unwrap();
    }
    let fallback = PathBuf::from("libs/external/core.ln");
    fallback.canonicalize().expect("Failed to locate canonical libs/external/core.ln")
}

/// Invariant CORE-1: Core provider name is never used as an implicit namespace,
/// including inside core.ln itself.
#[test]
fn test_invariant_core_1_no_synthetic_namespace_inside_core_source() {
    let core_path = locate_canonical_core_ln();
    let content = fs::read_to_string(&core_path).expect("Failed to read canonical core.ln");

    // Assert that "core::" does NOT appear anywhere in core.ln
    let occurrences: Vec<(usize, &str)> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("core::"))
        .collect();

    assert!(
        occurrences.is_empty(),
        "CORE-1 violation: core.ln must not use synthetic 'core::' namespace internally! Found in lines: {:?}",
        occurrences
    );

    // Verify key references inside core.ln are un-prefixed
    assert!(content.contains("Result::Ok("), "core.ln should reference Result::Ok directly");
    assert!(content.contains("Result::Err("), "core.ln should reference Result::Err directly");
    assert!(content.contains("Option::Some("), "core.ln should reference Option::Some directly");
    assert!(content.contains("Option::None"), "core.ln should reference Option::None directly");
    assert!(content.contains("ControlFlow::Continue("), "core.ln should reference ControlFlow::Continue directly");
    assert!(content.contains("ControlFlow::Break("), "core.ln should reference ControlFlow::Break directly");
}

/// Proof 1: Core source itself compiles
/// core.ln -> compiler -> core.llib
#[test]
fn test_proof_1_core_source_itself_compiles_to_llib() {
    let dir = create_temp_dir("proof_1_compile_core");
    let core_path = locate_canonical_core_ln();
    let core_src = fs::read_to_string(&core_path).expect("Failed to read core.ln");

    let out_llib = dir.join("core.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };

    let res = compile(core_path.to_str().unwrap(), core_src, &compile_opts);
    assert!(
        res.is_ok(),
        "Compiling core.ln to core.llib must succeed: {:?}",
        res.err()
    );
    assert!(out_llib.exists(), "core.llib artifact must exist on disk");
    assert!(
        fs::metadata(&out_llib).unwrap().len() > 0,
        "core.llib artifact must not be empty"
    );

    // Verify artifact is readable and contains valid metadata
    let mut file = fs::File::open(&out_llib).expect("Failed to open compiled core.llib");
    let (_mod, manifest, _obj, semantic_metadata) =
        MlibReader::read_module(&mut file).expect("Failed to parse core.llib with MlibReader");
    let manifest = manifest.expect("Manifest must exist in core.llib");
    assert_eq!(manifest.identity.version, "0.1.0");

    let meta = semantic_metadata.expect("core.llib must contain SemanticMetadata");
    let exported = &meta.interface.exported_symbols;

    assert!(exported.contains_key("Option"), "Option must be exported in core.llib");
    assert!(exported.contains_key("Result"), "Result must be exported in core.llib");
    assert!(exported.contains_key("ControlFlow"), "ControlFlow must be exported in core.llib");
    assert!(exported.contains_key("Drop"), "Drop must be exported in core.llib");
    assert!(exported.contains_key("Copy"), "Copy must be exported in core.llib");
    assert!(exported.contains_key("Try"), "Try must be exported in core.llib");
    assert!(exported.contains_key("FromResidual"), "FromResidual must be exported in core.llib");
    assert!(exported.contains_key("Iterator"), "Iterator must be exported in core.llib");
    assert!(exported.contains_key("Clone"), "Clone must be exported in core.llib");
    assert!(exported.contains_key("Eq"), "Eq must be exported in core.llib");
    assert!(exported.contains_key("Ord"), "Ord must be exported in core.llib");

    // Sync canonical libs/external/core.llib with freshly validated build
    let canonical_llib = core_path.with_file_name("core.llib");
    let _ = fs::copy(&out_llib, &canonical_llib);
}

/// Proof 2: Consumer can use root exports
/// import <core>;
/// dec x: Option<i32> = ...
/// dec y: Result<i32, i32> = ...
/// nhưng:
/// core::Option<i32> vẫn phải fail.
#[test]
fn test_proof_2_consumer_root_exports_direct_and_rejects_synthetic() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_2_root_exports");
    let main_path = dir.join("main.ln");

    // Positive case: direct un-prefixed exports work
    let valid_src = r#"
        import <core>;

        fn make_opt(x: i32) -> Option<i32> {
            return Option::Some(x);
        }

        fn make_res(x: i32) -> Result<i32, i32> {
            return Result::Ok(x);
        }

        fn main() -> i32 {
            dec x: Option<i32> = make_opt(42);
            dec y: Result<i32, i32> = make_res(100);
            return 0;
        }
    "#;
    fs::write(&main_path, valid_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), valid_src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Consumer with import <core> using bare Option and Result must succeed: {:?}",
        res.err()
    );

    // Negative case 1: core::Option must FAIL
    let bad_option_src = r#"
        import <core>;

        fn test_bad(x: core::Option<i32>) {}
    "#;
    fs::write(&main_path, bad_option_src).unwrap();
    let res_bad_opt = check(main_path.to_str().unwrap(), bad_option_src.to_string(), &opts);
    assert!(res_bad_opt.is_err(), "core::Option must fail under IMPORT-3");
    let errs_opt = res_bad_opt.unwrap_err();
    assert!(
        errs_opt.iter().any(|d| d.message.contains("core") || d.message.contains("unresolved")),
        "Expected unresolved error for core::Option, got: {:?}",
        errs_opt
    );

    // Negative case 2: core::Result must FAIL
    let bad_result_src = r#"
        import <core>;

        fn test_bad(x: core::Result<i32, i32>) {}
    "#;
    fs::write(&main_path, bad_result_src).unwrap();
    let res_bad_res = check(main_path.to_str().unwrap(), bad_result_src.to_string(), &opts);
    assert!(res_bad_res.is_err(), "core::Result must fail under IMPORT-3");
    let errs_res = res_bad_res.unwrap_err();
    assert!(
        errs_res.iter().any(|d| d.message.contains("core") || d.message.contains("unresolved")),
        "Expected unresolved error for core::Result, got: {:?}",
        errs_res
    );
}

/// Proof 3: Core semantic types survive full pipeline
/// AST -> Resolver -> TypeChecker -> Borrowck -> Mono -> MVIR -> LLVM
#[test]
fn test_proof_3_core_semantic_types_survive_full_pipeline() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_3_full_pipeline");
    let main_path = dir.join("main.ln");
    let out_obj = dir.join("main.obj");

    let consumer_src = r#"
        import <core>;

        struct Guard {
            id: i32,
        }

        impl Drop for Guard {
            fn drop(self: &rw Guard) {
                self.id = 0;
            }
        }

        fn process_option(opt: Option<i32>) -> Option<i32> {
            return opt;
        }

        fn process_result(res: Result<i32, i32>) -> Result<i32, i32> {
            return res;
        }

        fn main() -> i32 {
            dec g = Guard { id: 42 };
            dec o: Option<i32> = Option::Some(10);
            dec r: Result<i32, i32> = Result::Ok(20);
            dec r1 = process_option(o);
            dec r2 = process_result(r);
            return 0;
        }
    "#;
    fs::write(&main_path, consumer_src).unwrap();

    let compile_opts = CompilerOptions {
        output_path: Some(out_obj.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llvm: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(
        main_path.to_str().unwrap(),
        consumer_src.to_string(),
        &compile_opts,
    );

    assert!(
        compile_res.is_ok(),
        "Consumer using Core types through full pipeline (Borrowck, Mono, MVIR, LLVM) must succeed: {:?}",
        compile_res.err()
    );

    // Verify LLVM IR file was generated and contains expected symbols
    let ll_file = dir.join("main.ll");
    assert!(ll_file.exists(), "LLVM IR file main.ll must be generated");
    let ll_content = fs::read_to_string(&ll_file).expect("Failed to read main.ll");
    assert!(
        ll_content.contains("@process_option"),
        "LLVM IR must define process_option"
    );
    assert!(
        ll_content.contains("@process_result"),
        "LLVM IR must define process_result"
    );
    assert!(
        ll_content.contains("@__mellis_user_main"),
        "LLVM IR must define __mellis_user_main"
    );

    // Verify object file was generated
    assert!(out_obj.exists(), "Object file main.obj must be generated");
    assert!(
        fs::metadata(&out_obj).unwrap().len() > 0,
        "main.obj must not be empty"
    );
}

/// Proof 4: Source / .llib parity
/// import <core> resolved from core.ln vs resolved from core.llib
/// must produce equivalent semantic results.
#[test]
fn test_proof_4_source_vs_llib_parity() {
    let base_dir = create_temp_dir("proof_4_parity");
    let core_path = locate_canonical_core_ln();
    let core_src = fs::read_to_string(&core_path).expect("Failed to read core.ln");

    // Environment A: Source-only sysroot (core.ln only, no core.llib)
    let sysroot_a_dir = base_dir.join("sysroot_a");
    let ext_a = sysroot_a_dir.join("libs").join("external");
    fs::create_dir_all(&ext_a).unwrap();
    fs::write(ext_a.join("core.ln"), &core_src).unwrap();
    let sysroot_a = Sysroot::from_root(sysroot_a_dir.clone());

    // Environment B: Binary-only sysroot (core.llib only, no core.ln)
    let sysroot_b_dir = base_dir.join("sysroot_b");
    let ext_b = sysroot_b_dir.join("libs").join("external");
    fs::create_dir_all(&ext_b).unwrap();
    let llib_b_path = ext_b.join("core.llib");

    // Compile core.ln -> core.llib for Environment B
    let compile_core_opts = CompilerOptions {
        output_path: Some(llib_b_path.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let core_compile_res = compile(
        ext_a.join("core.ln").to_str().unwrap(),
        core_src.clone(),
        &compile_core_opts,
    );
    assert!(core_compile_res.is_ok(), "Compiling core.llib for parity test must succeed");
    assert!(llib_b_path.exists(), "core.llib must exist in sysroot_b");
    let sysroot_b = Sysroot::from_root(sysroot_b_dir.clone());

    let consumer_src = r#"
        import <core>;

        fn compute(flag: bool) -> Option<i32> {
            if flag {
                return Option::Some(42);
            }
            return Option::None;
        }

        fn run_pipeline() -> Result<i32, i32> {
            dec opt = compute(true);
            return Result::Ok(1);
        }

        fn main() -> i32 {
            dec res = run_pipeline();
            return 0;
        }
    "#;

    // Check under Sysroot A (Source mode)
    let consumer_a_dir = base_dir.join("consumer_a");
    fs::create_dir_all(&consumer_a_dir).unwrap();
    let main_a = consumer_a_dir.join("main.ln");
    fs::write(&main_a, consumer_src).unwrap();
    let opts_a = CompilerOptions {
        search_paths: vec![sysroot_a.root().to_string_lossy().to_string()],
        output_path: Some(consumer_a_dir.join("main.obj").to_string_lossy().to_string()),
        emit_llvm: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res_a = compile(main_a.to_str().unwrap(), consumer_src.to_string(), &opts_a);
    assert!(res_a.is_ok(), "Consumer under source sysroot A must succeed: {:?}", res_a.err());

    // Check under Sysroot B (Binary .llib mode)
    let consumer_b_dir = base_dir.join("consumer_b");
    fs::create_dir_all(&consumer_b_dir).unwrap();
    let main_b = consumer_b_dir.join("main.ln");
    fs::write(&main_b, consumer_src).unwrap();
    let opts_b = CompilerOptions {
        search_paths: vec![sysroot_b.root().to_string_lossy().to_string()],
        output_path: Some(consumer_b_dir.join("main.obj").to_string_lossy().to_string()),
        emit_llvm: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res_b = compile(main_b.to_str().unwrap(), consumer_src.to_string(), &opts_b);
    assert!(res_b.is_ok(), "Consumer under binary sysroot B must succeed: {:?}", res_b.err());

    // Compare generated LLVM IR definitions
    let ll_a = fs::read_to_string(consumer_a_dir.join("main.ll")).expect("Failed to read main.ll from A");
    let ll_b = fs::read_to_string(consumer_b_dir.join("main.ll")).expect("Failed to read main.ll from B");

    assert!(ll_a.contains("@compute"), "Source LLVM IR must contain compute");
    assert!(ll_b.contains("@compute"), "Binary LLVM IR must contain compute");
    assert!(ll_a.contains("@run_pipeline"), "Source LLVM IR must contain run_pipeline");
    assert!(ll_b.contains("@run_pipeline"), "Binary LLVM IR must contain run_pipeline");
}

/// Proof 5: No implicit prelude
/// Option, Result, Drop only appear because of `import <core>;`,
/// NOT because the compiler auto-injects Core into consumer scope.
#[test]
fn test_proof_5_no_implicit_prelude() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_5_no_implicit_prelude");
    let main_path = dir.join("main.ln");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // 1. Option cannot be used without import
    let src_option = "fn test_option(x: Option<i32>) {}";
    fs::write(&main_path, src_option).unwrap();
    let res_opt = check(main_path.to_str().unwrap(), src_option.to_string(), &opts);
    assert!(res_opt.is_err(), "Option must be unresolved without import <core>;");
    let errs_opt = res_opt.unwrap_err();
    assert!(
        errs_opt.iter().any(|d| d.message.contains("Option") || d.message.contains("unresolved")),
        "Expected unresolved error for Option, got: {:?}", errs_opt
    );

    // 2. Result cannot be used without import
    let src_result = "fn test_result(x: Result<i32, i32>) {}";
    fs::write(&main_path, src_result).unwrap();
    let res_res = check(main_path.to_str().unwrap(), src_result.to_string(), &opts);
    assert!(res_res.is_err(), "Result must be unresolved without import <core>;");
    let errs_res = res_res.unwrap_err();
    assert!(
        errs_res.iter().any(|d| d.message.contains("Result") || d.message.contains("unresolved")),
        "Expected unresolved error for Result, got: {:?}", errs_res
    );

    // 3. Drop trait cannot be used without import
    let src_drop = "fn test_drop(x: Drop) {}";
    fs::write(&main_path, src_drop).unwrap();
    let res_drop = check(main_path.to_str().unwrap(), src_drop.to_string(), &opts);
    assert!(res_drop.is_err(), "Drop must be unresolved without import <core>;");
    let errs_drop = res_drop.unwrap_err();
    assert!(
        errs_drop.iter().any(|d| d.message.contains("Drop") || d.message.contains("unresolved")),
        "Expected unresolved error for Drop, got: {:?}", errs_drop
    );

    // 4. ControlFlow cannot be used without import
    let src_cf = "fn test_cf(x: ControlFlow<i32, i32>) {}";
    fs::write(&main_path, src_cf).unwrap();
    let res_cf = check(main_path.to_str().unwrap(), src_cf.to_string(), &opts);
    assert!(res_cf.is_err(), "ControlFlow must be unresolved without import <core>;");
    let errs_cf = res_cf.unwrap_err();
    assert!(
        errs_cf.iter().any(|d| d.message.contains("ControlFlow") || d.message.contains("unresolved")),
        "Expected unresolved error for ControlFlow, got: {:?}", errs_cf
    );
}

/// Proof 6: Phase 4 Core Lifetime Adoption
/// Verify Option::as_ref and Result::as_ref tie the returned reference to the enum,
/// enforcing borrow checking soundness.
#[test]
fn test_proof_6_option_result_lifetime_projections() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_6_option_result_lifetime");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Case 1: Conflict - Mutating Option while as_ref borrow is active
    let conflict_src = r#"
import <core>;

fn main() {
    dec rw opt = Option::Some(42);
    dec r = opt.as_ref();
    opt = Option::None; // Conflict: opt is borrowed by r!
    dec _ = r;
}
"#;
    let conflict_path = dir.join("conflict.ln");
    fs::write(&conflict_path, conflict_src).unwrap();
    let res_conflict = check(conflict_path.to_str().unwrap(), conflict_src.to_string(), &opts);
    assert!(res_conflict.is_err(), "Mutating Option while as_ref reference is live must fail borrowck!");

    // Case 2: Safe - Mutating Option after as_ref borrow has ended
    let safe_src = r#"
import <core>;

fn main() {
    dec rw opt = Option::Some(42);
    dec r = opt.as_ref();
    dec _ = r;
    opt = Option::None; // Safe: r is dead!
}
"#;
    let safe_path = dir.join("safe.ln");
    fs::write(&safe_path, safe_src).unwrap();
    let res_safe = check(safe_path.to_str().unwrap(), safe_src.to_string(), &opts);
    assert!(res_safe.is_ok(), "Mutating Option after as_ref borrow ended must succeed! Got: {:?}", res_safe.err());

    // Case 3: Conflict - Mutating Result while as_ref borrow is active
    let res_conflict_src = r#"
import <core>;

fn main() {
    dec rw res: Result<i32, i32> = Result::Ok(100);
    dec r = res.as_ref();
    res = Result::Err(500); // Conflict: res is borrowed by r!
    dec _ = r;
}
"#;
    let res_conflict_path = dir.join("res_conflict.ln");
    fs::write(&res_conflict_path, res_conflict_src).unwrap();
    let res_c = check(res_conflict_path.to_str().unwrap(), res_conflict_src.to_string(), &opts);
    assert!(res_c.is_err(), "Mutating Result while as_ref reference is live must fail borrowck!");
}

