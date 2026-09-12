use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("mellis_provider_module_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

// =============================================================================
// CONTRACT 1: import <core> exposes root bindings; no synthetic core module.
// Option<i32> PASS, Result<i32, E> PASS, core::Option<i32> FAIL
// =============================================================================

#[test]
fn test_acceptance_core_root_bindings_pass() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("accept_core_root_bindings_pass");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;

        fn test_opt(x: Option<i32>) -> Option<i32> {
            return x;
        }

        fn test_res<E>(x: Result<i32, E>) -> Result<i32, E> {
            return x;
        }

        fn main() {
            dec opt: Option<i32> = Option::Some(42);
            dec res: Result<i32, i32> = Result::Ok(100);
            dec o = test_opt(opt);
            dec r = test_res(res);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Direct root Option<i32> and Result<i32, E> from <core> must succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_acceptance_core_synthetic_namespace_fails() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("accept_core_synthetic_namespace_fails");

    // core::Option<i32> must FAIL
    let main_opt = dir.join("main_opt.ln");
    let src_opt = r#"
        import <core>;
        fn main() {
            dec x: core::Option<i32>;
        }
    "#;
    fs::write(&main_opt, src_opt).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res_opt = check(main_opt.to_str().unwrap(), src_opt.to_string(), &opts);
    assert!(
        res_opt.is_err(),
        "core::Option<i32> must fail: provider 'core' is not a module namespace"
    );
    let errs_opt = res_opt.unwrap_err();
    assert!(
        errs_opt
            .iter()
            .any(|d| d.message.contains("core") || d.message.contains("not found")),
        "Expected error for core::Option, got: {:?}",
        errs_opt
    );

    // core::Result<i32, i32> must FAIL
    let main_res = dir.join("main_res.ln");
    let src_res = r#"
        import <core>;
        fn main() {
            dec r: core::Result<i32, i32>;
        }
    "#;
    fs::write(&main_res, src_res).unwrap();
    let res_res = check(main_res.to_str().unwrap(), src_res.to_string(), &opts);
    assert!(
        res_res.is_err(),
        "core::Result<i32, i32> must fail: provider 'core' is not a module namespace"
    );
}

// =============================================================================
// CONTRACT 2: Provider foo with explicit module math
// math::add(...) PASS, foo::math::add(...) FAIL
// =============================================================================

#[test]
fn test_acceptance_provider_explicit_module_pass() {
    let dir = create_temp_dir("accept_provider_explicit_module_pass");
    let foo_path = dir.join("foo.ln");
    fs::write(
        &foo_path,
        r#"
        module math {
            export fn add(a: i32, b: i32) -> i32 {
                return a + b;
            }
        }
    "#,
    )
    .unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "foo";
        fn main() {
            dec res = math::add(10, 20);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "math::add(...) from provider 'foo' must succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_acceptance_provider_prefix_fails() {
    let dir = create_temp_dir("accept_provider_prefix_fails");
    let foo_path = dir.join("foo.ln");
    fs::write(
        &foo_path,
        r#"
        module math {
            export fn add(a: i32, b: i32) -> i32 {
                return a + b;
            }
        }
    "#,
    )
    .unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "foo";
        fn main() {
            dec res = foo::math::add(10, 20);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "foo::math::add(...) must fail: provider 'foo' cannot be used as module prefix"
    );
    let errs = res.unwrap_err();
    assert!(
        errs.iter()
            .any(|d| d.message.contains("foo") || d.message.contains("not found")),
        "Expected error for foo::math::add, got: {:?}",
        errs
    );
}

// =============================================================================
// CONTRACT 3: Same provider imported twice is strictly idempotent.
// =============================================================================

#[test]
fn test_acceptance_same_provider_imported_twice_idempotent() {
    let dir = create_temp_dir("accept_same_provider_twice_idempotent");
    let foo_path = dir.join("foo.ln");
    fs::write(
        &foo_path,
        r#"
        export fn root_calc(x: i32) -> i32 {
            return x * 2;
        }
        module utils {
            export fn answer() -> i32 {
                return 42;
            }
        }
    "#,
    )
    .unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "foo";
        import "foo";
        import "foo";
        fn main() {
            dec a = root_calc(10);
            dec b = utils::answer();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "Importing same provider multiple times must be strictly idempotent: {:?}",
        res.err()
    );
}

// =============================================================================
// CONTRACT 4: Provider A + Provider B export same symbol X -> E1002 collision
// =============================================================================

#[test]
fn test_acceptance_provider_collision_e1002() {
    let dir = create_temp_dir("accept_provider_collision_e1002");
    let a_path = dir.join("prov_a.ln");
    fs::write(
        &a_path,
        r#"
        export fn duplicate_symbol() -> i32 {
            return 1;
        }
    "#,
    )
    .unwrap();

    let b_path = dir.join("prov_b.ln");
    fs::write(
        &b_path,
        r#"
        export fn duplicate_symbol() -> i32 {
            return 2;
        }
    "#,
    )
    .unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "prov_a";
        import "prov_b";
        fn main() {
            dec x = duplicate_symbol();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "Colliding exports from distinct providers must fail"
    );
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("E1002")),
        "Expected E1002 diagnostic for symbol collision, got: {:?}",
        errs
    );
}

// =============================================================================
// CONTRACT 5: Private provider symbols are inaccessible in consumer
// =============================================================================

#[test]
fn test_acceptance_private_export_inaccessible() {
    let dir = create_temp_dir("accept_private_inaccessible");
    let prov_path = dir.join("prov.ln");
    fs::write(
        &prov_path,
        r#"
        fn private_root() -> i32 { return 100; }
        export fn public_root() -> i32 { return private_root(); }

        module secret_mod {
            fn private_inner() -> i32 { return 200; }
            export fn public_inner() -> i32 { return private_inner(); }
        }
    "#,
    )
    .unwrap();

    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // 1. Private root symbol must not be accessible
    let main_priv_root = dir.join("main_priv_root.ln");
    let src_priv_root = r#"
        import "prov";
        fn main() {
            dec x = private_root();
        }
    "#;
    fs::write(&main_priv_root, src_priv_root).unwrap();
    let res_priv_root = check(
        main_priv_root.to_str().unwrap(),
        src_priv_root.to_string(),
        &opts,
    );
    assert!(
        res_priv_root.is_err(),
        "private_root() must not be accessible in consumer"
    );

    // 2. Private inner symbol inside module must not be accessible
    let main_priv_inner = dir.join("main_priv_inner.ln");
    let src_priv_inner = r#"
        import "prov";
        fn main() {
            dec x = secret_mod::private_inner();
        }
    "#;
    fs::write(&main_priv_inner, src_priv_inner).unwrap();
    let res_priv_inner = check(
        main_priv_inner.to_str().unwrap(),
        src_priv_inner.to_string(),
        &opts,
    );
    assert!(
        res_priv_inner.is_err(),
        "secret_mod::private_inner() must not be accessible in consumer"
    );

    // 3. Public symbols must succeed
    let main_pub = dir.join("main_pub.ln");
    let src_pub = r#"
        import "prov";
        fn main() {
            dec a = public_root();
            dec b = secret_mod::public_inner();
        }
    "#;
    fs::write(&main_pub, src_pub).unwrap();
    let res_pub = check(main_pub.to_str().unwrap(), src_pub.to_string(), &opts);
    assert!(
        res_pub.is_ok(),
        "public_root() and secret_mod::public_inner() must succeed: {:?}",
        res_pub.err()
    );
}

// =============================================================================
// CONTRACT 6: Source (.ln) vs Binary (.llib) Parity with Identical Namespace
// =============================================================================

#[test]
fn test_acceptance_source_vs_llib_identical_namespace() {
    let dir = create_temp_dir("accept_source_vs_llib_identical_namespace");
    let prov_path = dir.join("shapes.ln");
    let prov_src = r#"
        export fn calculate_diameter(r: i32) -> i32 {
            return r * 2;
        }

        module geometry {
            export struct Circle {
                radius: i32,
            }
            export fn make_circle(r: i32) -> Circle {
                return Circle { radius: r };
            }
            export fn get_radius(c: &Circle) -> i32 {
                return c.radius;
            }
        }
    "#;
    fs::write(&prov_path, prov_src).unwrap();

    let consumer_src = r#"
        import "shapes";
        fn main() {
            dec d = calculate_diameter(10);
            dec c = geometry::make_circle(5);
            dec r = geometry::get_radius(&c);
        }
    "#;
    let main_path = dir.join("main.ln");
    fs::write(&main_path, consumer_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // 1. Validate in Source mode (.ln)
    let res_source = check(
        main_path.to_str().unwrap(),
        consumer_src.to_string(),
        &opts,
    );
    assert!(
        res_source.is_ok(),
        "Consumer against source .ln provider must succeed: {:?}",
        res_source.err()
    );

    // 2. Compile provider shapes.ln into shapes.llib
    let llib_path = dir.join("shapes.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let compile_res = compile(
        prov_path.to_str().unwrap(),
        prov_src.to_string(),
        &compile_opts,
    );
    assert!(
        compile_res.is_ok(),
        "Compiling shapes.ln to shapes.llib must succeed: {:?}",
        compile_res.err()
    );
    assert!(llib_path.exists(), "shapes.llib artifact must exist");

    // 3. Delete shapes.ln so loader MUST resolve and load shapes.llib
    fs::remove_file(&prov_path).unwrap();

    // 4. Validate in Binary mode (.llib) - must have identical namespace resolution
    let res_llib = check(
        main_path.to_str().unwrap(),
        consumer_src.to_string(),
        &opts,
    );
    assert!(
        res_llib.is_ok(),
        "Consumer against binary .llib provider must succeed with identical namespace: {:?}",
        res_llib.err()
    );
}
