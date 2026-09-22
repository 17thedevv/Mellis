use luna_common::CompilerSession;
use luna_driver::resolution_context::ProviderResolutionContext;
use luna_driver::sysroot::Sysroot;
use luna_driver::{check, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("mellis_core_isolation_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn check_with_sysroot(test_name: &str, source: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = Sysroot::discover_for_test().expect("test sysroot required");
    let dir = create_temp_dir(test_name);
    let path = dir.join("main.ln");
    fs::write(&path, source).unwrap();
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    check(path.to_str().unwrap(), source.to_string(), &options)
}

/// Compiler-required contracts are visible without a user import, but ordinary
/// core APIs such as Result remain explicit.
#[test]
fn test_language_contract_visibility_is_controlled() {
    let contracts = r#"
        fn accepts_drop<T: Drop>(value: &T) {}
        fn accepts_iterator<I: Iterator>(value: &I) {}
        fn accepts_into_iterator<I: IntoIterator>(value: &I) {}
        fn main() {
            dec value: Option<i32> = Option::Some(1);
        }
    "#;
    assert!(
        check_with_sysroot("implicit_contracts", contracts).is_ok(),
        "Drop, Option, Iterator and IntoIterator are compiler contracts"
    );

    let ordinary_api = "fn main() { dec value: Result<i32, i32> = Result::Ok(1); }";
    assert!(
        check_with_sysroot("result_not_implicit", ordinary_api).is_err(),
        "Result must not become implicitly visible"
    );
}

#[test]
fn test_explicit_result_provider_exposes_root_binding_only() {
    let good = r#"
        import <result>;
        fn main() { dec value: Result<i32, i32> = Result::Ok(42); }
    "#;
    assert!(check_with_sysroot("explicit_result", good).is_ok());

    let bad = r#"
        import <result>;
        fn main() { dec value: result::Result<i32, i32>; }
    "#;
    assert!(
        check_with_sysroot("result_not_namespace", bad).is_err(),
        "logical provider identity must not synthesize a namespace"
    );
}

#[test]
fn test_try_operator_works_through_explicit_try_contract_provider() {
    let source = r#"
        import <result>;
        import <try>;

        fn step1() -> Result<i32, i32> { return Result::Ok(10); }
        fn step2() -> Result<i32, i32> {
            dec value = step1()?;
            return Result::Ok(value + 5);
        }
        fn main() {}
    "#;
    assert!(
        check_with_sysroot("explicit_try", source).is_ok(),
        "the try surface is an explicit logical provider"
    );
}

#[test]
fn test_drop_contract_is_available_without_ordinary_core_imports() {
    let source = r#"
        struct Handle { id: i32, };

        impl Drop for Handle {
            fn drop(self: &rw Handle) {}
        }
        fn main() { dec handle = Handle { id: 1 }; }
    "#;
    assert!(check_with_sysroot("implicit_drop", source).is_ok());
}

#[test]
fn test_lang_item_registry_uses_component_provider_declarations() {
    use luna_semantic::lang_item::LangItem;

    let sysroot = Sysroot::discover_for_test().expect("test sysroot required");
    let mut compiler_session = CompilerSession::new();
    let mut driver_session =
        luna_driver::session::DriverSession::new(sysroot, &mut compiler_session, &[]);
    let mut arena = luna_ast::AstArena::new();

    driver_session
        .bootstrap_lang_contracts(&mut arena)
        .expect("language contract bootstrap must succeed");
    driver_session
        .load_package("try", &mut arena, ProviderResolutionContext::SysrootDependency)
        .expect("try provider must load through the manifest");

    let mut semantic_ctx = luna_semantic::SemanticContext::new();
    driver_session.registry.inject_into_ctx(&mut semantic_ctx);

    for item in [
        LangItem::Drop,
        LangItem::Option,
        LangItem::OptionSome,
        LangItem::OptionNone,
        LangItem::Iterator,
        LangItem::IntoIterator,
        LangItem::FromResidual,
        LangItem::Try,
        LangItem::ControlFlow,
        LangItem::ControlFlowContinue,
        LangItem::ControlFlowBreak,
    ] {
        assert!(
            semantic_ctx.lang_items.get(item).is_some(),
            "missing canonical lang item {item:?}"
        );
    }
}

#[test]
fn test_option_ext_method_available_without_import_result() {
    let src = r#"
        fn main() {
            dec opt = Option::Some(10);
            dec r = opt.ok_or(0);
        }
    "#;
    assert!(
        check_with_sysroot("option_ext_ok_or_available", src).is_ok(),
        "OptionExt method ok_or must be available through controlled standard prelude without importing <result>"
    );
}

#[test]
fn test_result_type_unresolved_without_import_result() {
    let src = r#"
        fn main() {
            dec r: Result<i32, i32>;
        }
    "#;
    assert!(
        check_with_sysroot("result_type_unresolved", src).is_err(),
        "Result type must be unresolved without explicit import <result>"
    );
}

#[test]
fn test_result_type_resolved_with_explicit_import_result() {
    let src = r#"
        import <result>;
        fn main() {
            dec r: Result<i32, i32> = Result::Ok(42);
        }
    "#;
    assert!(
        check_with_sysroot("result_type_resolved", src).is_ok(),
        "Result must resolve with explicit import <result>"
    );
}

#[test]
fn test_transitive_bootstrap_panic_not_globally_visible() {
    let bad = r#"
        fn main() -> i32 {
            __luna_panic_default();
        }
    "#;
    assert!(
        check_with_sysroot("panic_not_globally_visible", bad).is_err(),
        "__luna_panic_default must not be globally visible despite core/panic being transitively loaded during bootstrap"
    );

    let good = r#"
        import <core/panic>;
        fn main() -> i32 {
            __luna_panic_default();
        }
    "#;
    assert!(
        check_with_sysroot("panic_with_explicit_import", good).is_ok(),
        "__luna_panic_default must be available with explicit import <core/panic>"
    );
}


#[test]
fn test_explicit_standard_providers_negative_matrix() {
    struct Case {
        name: &'static str,
        provider: &'static str,
        bad_snippet: &'static str,
        good_snippet: &'static str,
    };

    let cases = [
        Case {
            name: "vec",
            provider: "vec",
            bad_snippet: "fn main() { dec v = vec_new<i32>(); }",
            good_snippet: "import <vec>; fn main() { dec v = vec_new<i32>(); }",
        },
        Case {
            name: "string",
            provider: "string",
            bad_snippet: "fn main() { dec s = string_new(); }",
            good_snippet: "import <string>; fn main() { dec s = string_new(); }",
        },
        Case {
            name: "hashmap",
            provider: "hashmap",
            bad_snippet: "fn main() { dec m = hashmap_new<i32, i32>(); }",
            good_snippet: "import <hashmap>; fn main() { dec m = hashmap_new<i32, i32>(); }",
        },
        Case {
            name: "hashset",
            provider: "hashset",
            bad_snippet: "fn main() { dec s = hashset_new<i32>(); }",
            good_snippet: "import <hashset>; fn main() { dec s = hashset_new<i32>(); }",
        },
        Case {
            name: "io",
            provider: "io",
            bad_snippet: "fn main() { dec msg: [u8; 1] = [65 as u8]; io::println(&msg); }",
            good_snippet: "import <io>; fn main() { dec msg: [u8; 1] = [65 as u8]; io::println(&msg); }",
        },
    ];

    for case in cases {
        assert!(
            check_with_sysroot(&format!("neg_{}", case.name), case.bad_snippet).is_err(),
            "{} must require explicit import <{}>",
            case.name,
            case.provider
        );
        assert!(
            check_with_sysroot(&format!("pos_{}", case.name), case.good_snippet).is_ok(),
            "{} with import <{}> must resolve cleanly",
            case.name,
            case.provider
        );
    }
}

