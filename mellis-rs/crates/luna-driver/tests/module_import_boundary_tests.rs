use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_module_import_boundary_tests").join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

// ----------------------------------------------------
// M1: Disjoint Resolution Domains
// ----------------------------------------------------

#[test]
fn test_m1_local_rejects_external() {
    let dir = create_temp_dir("m1_local_rejects_ext");
    let main_path = dir.join("main.ln");
    let src = r#"
        import "core";
        fn main() {}
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "import \"core\" must NOT resolve external core sysroot package");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("Could not resolve module provider 'core'") || d.message.contains("not found")),
        "Expected error resolving module provider 'core', got: {:?}", errs
    );
}

#[test]
fn test_m1_external_rejects_local() {
    let dir = create_temp_dir("m1_ext_rejects_loc");
    let loc_path = dir.join("local_only.ln");
    fs::write(&loc_path, "module local_only { export fn helper() -> i32 { return 42; } }").unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import <local_only>;
        fn main() {}
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "import <local_only> must NOT resolve local search paths");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("external component") && d.message.contains("not found")),
        "Expected external component not found, got: {:?}", errs
    );
}

#[test]
fn test_m1_local_accepts_local() {
    let dir = create_temp_dir("m1_loc_accepts_loc");
    let loc_path = dir.join("mylocal.ln");
    fs::write(&loc_path, "module mylocal { export fn answer() -> i32 { return 42; } }").unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "mylocal";
        fn main() {
            dec x = mylocal::answer();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "import \"mylocal\" must succeed: {:?}", res.err());
}

#[test]
fn test_m1_external_accepts_external() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("m1_ext_accepts_ext");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() {}
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "import <core> must succeed: {:?}", res.err());
}

// ----------------------------------------------------
// M2: Module Traversal Across Compiler Phases
// ----------------------------------------------------

#[test]
fn test_m2_module_fn_traversal() {
    let dir = create_temp_dir("m2_fn");
    let main_path = dir.join("main.ln");
    let src = r#"
        module mymod {
            export fn compute(x: i32) -> i32 {
                return x * 2;
            }
        }
        fn main() {
            dec res = mymod::compute(21);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "module fn must be checked: {:?}", res.err());
}

#[test]
fn test_m2_nested_module_fn_traversal() {
    let dir = create_temp_dir("m2_nested");
    let main_path = dir.join("main.ln");
    let src = r#"
        module outer {
            export module inner {
                export fn deep() -> i32 { return 100; }
            }
        }
        fn main() {
            dec d = outer::inner::deep();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "nested module fn must be checked: {:?}", res.err());
}

#[test]
fn test_m2_module_generic_fn() {
    let dir = create_temp_dir("m2_generic");
    let main_path = dir.join("main.ln");
    let src = r#"
        module utils {
            export fn id<T>(x: T) -> T {
                return x;
            }
        }
        fn main() {
            dec a = utils::id<i32>(42);
            dec b = utils::id<bool>(true);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "module generic fn monomorphization must succeed: {:?}", res.err());
}

#[test]
fn test_m2_module_struct_and_impl() {
    let dir = create_temp_dir("m2_struct_impl");
    let main_path = dir.join("main.ln");
    let src = r#"
        module shapes {
            export struct Point {
                x: i32,
                y: i32,
            }
            export trait Describable {
                fn val(self: &Self) -> i32;
            }
            export impl Describable for Point {
                fn val(self: &Point) -> i32 {
                    return self.x + self.y;
                }
            }
        }
        fn main() {
            dec pt = shapes::Point { x: 10, y: 20 };
            dec sum = pt.val();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "module struct, trait, and impl must succeed: {:?}", res.err());
}

#[test]
fn test_m2_module_const() {
    let dir = create_temp_dir("m2_const");
    let main_path = dir.join("main.ln");
    let src = r#"
        module config {
            export dec MAX_SIZE: i32 = 1024;
        }
        fn main() {
            dec m = config::MAX_SIZE;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "module const must succeed: {:?}", res.err());
}

// ----------------------------------------------------
// M3: Multi-Provider Namespace Aggregation
// ----------------------------------------------------

#[test]
fn test_m3_multi_provider_aggregation() {
    let dir = create_temp_dir("m3_multi_provider");
    let p_a = dir.join("prov_a.ln");
    fs::write(&p_a, "module mymath { export fn add(a: i32, b: i32) -> i32 { return a + b; } }").unwrap();

    let p_b = dir.join("prov_b.ln");
    fs::write(&p_b, "module mymath { export fn sub(a: i32, b: i32) -> i32 { return a - b; } }").unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "prov_a";
        import "prov_b";
        fn main() {
            dec x = mymath::add(10, 5);
            dec y = mymath::sub(10, 5);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "multi-provider namespace aggregation must succeed: {:?}", res.err());
}

// ----------------------------------------------------
// M4: Visibility Enforcement
// ----------------------------------------------------

#[test]
fn test_m4_private_fn_rejected() {
    let dir = create_temp_dir("m4_priv_fn");
    let main_path = dir.join("main.ln");
    let src = r#"
        module secret {
            fn private_worker() -> i32 { return 42; }
        }
        fn main() {
            dec x = secret::private_worker();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "private function must be rejected from outside");
    let errs = res.unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("private")), "Expected 'private' error, got: {:?}", errs);
}

#[test]
fn test_m4_same_module_private_fn_accepted() {
    let dir = create_temp_dir("m4_same_mod_fn");
    let main_path = dir.join("main.ln");
    let src = r#"
        module secret {
            fn private_worker() -> i32 { return 42; }
            export fn public_api() -> i32 {
                return private_worker();
            }
        }
        fn main() {
            dec x = secret::public_api();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "private function inside same module must be accessible: {:?}", res.err());
}

#[test]
fn test_m4_private_struct_rejected() {
    let dir = create_temp_dir("m4_priv_struct");
    let main_path = dir.join("main.ln");
    let src = r#"
        module data {
            struct SecretData {
                key: i32,
            }
        }
        fn main() {
            dec s: data::SecretData;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "private struct must be rejected from outside");
    let errs = res.unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("private")), "Expected 'private' error, got: {:?}", errs);
}

#[test]
fn test_m4_private_method_rejected() {
    let dir = create_temp_dir("m4_priv_method");
    let main_path = dir.join("main.ln");
    let src = r#"
        module engine {
            export struct Motor {
                power: i32,
            }
            export impl Motor {
                fn internal_spark(self: &Motor) -> i32 { return self.power; }
                export fn start(self: &Motor) -> i32 { return self.internal_spark(); }
            }
        }
        fn main() {
            dec m = engine::Motor { power: 100 };
            dec s = m.internal_spark();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "private method must be rejected from outside");
    let errs = res.unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("private")), "Expected 'private' error, got: {:?}", errs);
}

#[test]
fn test_m4_cross_provider_private_rejected() {
    let dir = create_temp_dir("m4_cross_provider_priv");
    let p_sec = dir.join("prov_secret.ln");
    fs::write(&p_sec, r#"
        module secmod {
            fn internal_key() -> i32 { return 999; }
            export fn get_key() -> i32 { return internal_key(); }
        }
    "#).unwrap();

    let main_path = dir.join("main.ln");
    let src = r#"
        import "prov_secret";
        fn main() {
            dec k = secmod::internal_key();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "cross-provider private symbol must not be accessible");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("not found") || d.message.contains("private")),
        "Expected not found or private error, got: {:?}", errs
    );
}

// ----------------------------------------------------
// M5: Unsupported Syntax Rejection
// ----------------------------------------------------

#[test]
fn test_m5_export_import_rejected() {
    let dir = create_temp_dir("m5_export_import");
    let main_path = dir.join("main.ln");
    let src = "export import <core>;\nfn main() {}";
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "export import must be rejected");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("import` declarations cannot be exported")),
        "Expected export import diagnostic, got: {:?}", errs
    );
}

#[test]
fn test_m5_export_using_rejected() {
    let dir = create_temp_dir("m5_export_using");
    let main_path = dir.join("main.ln");
    let src = "export using core as c;\nfn main() {}";
    fs::write(&main_path, src).unwrap();
    let opts = CompilerOptions { quiet: true, ..Default::default() };
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "export using must be rejected");
    let errs = res.unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("using` aliases cannot be exported")),
        "Expected export using diagnostic, got: {:?}", errs
    );
}

// ----------------------------------------------------
// Negative Invariant: Diagnostic -> No Artifact Produced
// ----------------------------------------------------

#[test]
fn test_negative_invariant_no_artifact_on_error() {
    let dir = create_temp_dir("neg_invariant_artifact");
    let main_path = dir.join("main.ln");
    let src = "import \"nonexistent_mod\";\nfn main() {}";
    fs::write(&main_path, src).unwrap();

    let out_exe = dir.join("output.exe");
    let out_obj = dir.join("output.obj");
    let out_mlib = dir.join("output.mlib");

    let opts = CompilerOptions {
        output_path: Some(out_exe.to_string_lossy().to_string()),
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Compilation with unresolved module must fail");
    assert!(!out_exe.exists(), "Executable artifact must NOT exist upon diagnostic error");
    assert!(!out_obj.exists(), "Object artifact must NOT exist upon diagnostic error");
    assert!(!out_mlib.exists(), "Mlib artifact must NOT exist upon diagnostic error");
}
