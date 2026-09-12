use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_import_invariants_tests").join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

/// IMPORT-3: Provider name NEVER automatically creates a module namespace.
/// `import <core>; core::Option` must be REJECTED.
/// Bare `Option` must SUCCEED.
#[test]
fn test_import_3_no_synthetic_modules() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("import_3_no_synthetic");
    let main_fail = dir.join("main_fail.ln");
    let src_fail = r#"
        import <core>;
        fn main() {
            dec opt: core::Option<i32> = core::Option::None;
        }
    "#;
    fs::write(&main_fail, src_fail).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res_fail = check(main_fail.to_str().unwrap(), src_fail.to_string(), &opts);
    assert!(res_fail.is_err(), "core::Option must be rejected as core is a provider, not a module namespace");
    let errs = res_fail.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("core") || d.message.contains("not found") || d.message.contains("unresolved")),
        "Expected error mentioning unresolved namespace or symbol core, got: {:?}", errs
    );

    // Verify that bare Option succeeds
    let main_ok = dir.join("main_ok.ln");
    let src_ok = r#"
        import <core>;
        fn main() {
            dec opt: Option<i32> = Option::None;
        }
    "#;
    fs::write(&main_ok, src_ok).unwrap();
    let res_ok = check(main_ok.to_str().unwrap(), src_ok.to_string(), &opts);
    assert!(res_ok.is_ok(), "Bare Option<i32> must succeed: {:?}", res_ok.err());
}

/// IMPORT-2 & IMPORT-7: Explicit module namespaces are preserved.
/// `module foo { export struct Bar {} }` -> `foo::Bar` succeeds, bare `Bar` fails.
#[test]
fn test_import_2_and_7_hierarchy_preservation() {
    let dir = create_temp_dir("import_2_7_hierarchy");
    let prov_path = dir.join("prov_mod.ln");
    let prov_src = r#"
        module foo {
            export struct Bar {
                export x: i32,
            }
        }
    "#;
    fs::write(&prov_path, prov_src).unwrap();

    let main_ok = dir.join("main_ok.ln");
    let src_ok = r#"
        import "prov_mod";
        fn main() {
            dec b: foo::Bar = foo::Bar { x: 42 };
        }
    "#;
    fs::write(&main_ok, src_ok).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res_ok = check(main_ok.to_str().unwrap(), src_ok.to_string(), &opts);
    assert!(res_ok.is_ok(), "foo::Bar must succeed: {:?}", res_ok.err());

    let main_fail = dir.join("main_fail.ln");
    let src_fail = r#"
        import "prov_mod";
        fn main() {
            dec b: Bar = Bar { x: 42 };
        }
    "#;
    fs::write(&main_fail, src_fail).unwrap();
    let res_fail = check(main_fail.to_str().unwrap(), src_fail.to_string(), &opts);
    assert!(res_fail.is_err(), "Bare Bar must be rejected because it is inside module foo");
}

/// IMPORT-6: Private provider symbols are never injected into consumer scopes.
#[test]
fn test_import_6_visibility_enforcement() {
    let dir = create_temp_dir("import_6_visibility");
    let prov_path = dir.join("prov_vis.ln");
    let prov_src = r#"
        export struct PublicItem {
            export x: i32,
        }
        struct PrivateItem {
            y: i32,
        }
        fn private_helper() -> i32 {
            return 123;
        }
    "#;
    fs::write(&prov_path, prov_src).unwrap();

    let main_ok = dir.join("main_ok.ln");
    let src_ok = r#"
        import "prov_vis";
        fn main() {
            dec p: PublicItem = PublicItem { x: 1 };
        }
    "#;
    fs::write(&main_ok, src_ok).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res_ok = check(main_ok.to_str().unwrap(), src_ok.to_string(), &opts);
    assert!(res_ok.is_ok(), "PublicItem must succeed: {:?}", res_ok.err());

    let main_fail = dir.join("main_fail.ln");
    let src_fail = r#"
        import "prov_vis";
        fn main() {
            dec p: PrivateItem = PrivateItem { y: 2 };
        }
    "#;
    fs::write(&main_fail, src_fail).unwrap();
    let res_fail = check(main_fail.to_str().unwrap(), src_fail.to_string(), &opts);
    assert!(res_fail.is_err(), "PrivateItem must not be imported into consumer scope");
}

/// IMPORT-4: Repeated import of the same provider is strictly idempotent.
#[test]
fn test_import_4_idempotent_reimport() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("import_4_idempotent");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        import <core>;
        import <core>;
        import <alloc>;
        import <alloc>;
        fn main() {
            dec opt: Option<i32> = Option::None;
            dec rw v: Vec<i32> = vec_new<i32>();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Repeated imports of core and alloc must be strictly idempotent: {:?}", res.err());
}

/// IMPORT-5: Conflicting imported root bindings from distinct providers trigger deterministic duplicate definition error.
#[test]
fn test_import_5_deterministic_collision() {
    let dir = create_temp_dir("import_5_collision");
    let prov_a = dir.join("prov_a.ln");
    fs::write(&prov_a, "export struct ConflictingStruct { a: i32, }").unwrap();

    let prov_b = dir.join("prov_b.ln");
    fs::write(&prov_b, "export struct ConflictingStruct { b: i32, }").unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "prov_a";
        import "prov_b";
        fn main() {
            dec c: ConflictingStruct;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Conflicting root exports from prov_a and prov_b must trigger error");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("Duplicate") || d.message.contains("duplicate") || d.message.contains("ConflictingStruct") || d.message.contains("1002")),
        "Expected duplicate definition diagnostic, got: {:?}", errs
    );
}

/// IMPORT-8: Source (.ln) vs Binary (.llib) Parity Test.
/// Loading from .ln and from .llib produces identical namespace resolution and checking.
#[test]
fn test_import_8_source_vs_binary_parity() {
    let dir = create_temp_dir("import_8_parity");
    let prov_path = dir.join("parity_lib.ln");
    let prov_src = r#"
        export struct Widget<T> {
            val: T,
        }
        impl<T> Widget<T> {
            export fn get(self: &Self) -> T {
                return self.val;
            }
        }
        export fn make_widget<T>(x: T) -> Widget<T> {
            return Widget<T> { val: x };
        }
    "#;
    fs::write(&prov_path, prov_src).unwrap();

    let consumer_src = r#"
        import "parity_lib";
        fn main() -> i32 {
            dec w: Widget<i32> = make_widget<i32>(42);
            return w.get();
        }
    "#;
    let consumer_path = dir.join("consumer.ln");
    fs::write(&consumer_path, consumer_src).unwrap();

    let opts_src = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // 1. Check with source provider (.ln)
    let res_src = check(consumer_path.to_str().unwrap(), consumer_src.to_string(), &opts_src);
    assert!(res_src.is_ok(), "Source mode check failed: {:?}", res_src.err());

    // 2. Compile provider to .llib
    let llib_path = dir.join("parity_lib.llib");
    let opts_build_lib = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res_lib = compile(prov_path.to_str().unwrap(), prov_src.to_string(), &opts_build_lib);
    assert!(res_lib.is_ok(), "Failed to build parity_lib.llib: {:?}", res_lib.err());
    assert!(llib_path.exists(), "parity_lib.llib was not created");

    // Hide the source .ln so consumer MUST resolve from .llib
    let hidden_prov = dir.join("parity_lib.ln.bak");
    fs::rename(&prov_path, &hidden_prov).unwrap();

    // 3. Check with binary provider (.llib)
    let res_bin = check(consumer_path.to_str().unwrap(), consumer_src.to_string(), &opts_src);
    assert!(res_bin.is_ok(), "Binary mode check failed with .llib: {:?}", res_bin.err());

    // Restore provider source
    fs::rename(&hidden_prov, &prov_path).unwrap();
}
