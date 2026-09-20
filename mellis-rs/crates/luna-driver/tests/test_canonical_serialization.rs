use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_executable(exe_path: &Path) -> (i32, String, String) {
    let output = Command::new(exe_path)
        .output()
        .expect("Failed to execute compiled binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_canon_ser_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

/// Invariant: Cross-provider symbol identity MUST preserve full logical namespace path.
/// A local declaration name alone is never a valid globally stable symbol identity.
///
/// Scenario:
/// Provider `items_def` defines:
///   module mod_a { export struct Item { val: i32 } }
///   module mod_b { export struct Item { val: i32 } }
///   export type Item = mod_b::Item;
///
/// Provider `bridge` imports `items_def` and exports functions returning `mod_a::Item` and `mod_b::Item`.
/// Both `items_def` and `bridge` are compiled to `.llib`.
///
/// Consumer `main` imports `items_def` and `bridge`, calls `produce_a()` and `produce_b()`.
/// Consumer MUST resolve precisely `mod_a::Item` (with `get_a()`) and `mod_b::Item` (with `get_b()`),
/// without collision with each other or the top-level alias `Item`.
#[test]
fn test_cross_provider_module_scoped_symbol_identity_preserved_in_llib() {
    let dir = create_temp_dir("module_scoped_identity");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // 1. Compile items_def.ln -> items_def.llib
    let defs_path = dir.join("items_def.ln");
    let defs_llib = dir.join("items_def.llib");
    let defs_src = r#"
        module mod_a {
            export struct Item {
                x: i32,
            };

            export fn make_a(v: i32) -> Item {
                return Item { x: v };
            }

            export fn check_a(it: &Item) -> i32 {
                return it.x;
            }
        }

        module mod_b {
            export struct Item {
                y: i32,
                z: i32,
            };

            export fn make_b(v: i32) -> Item {
                return Item { y: v, z: v * 2 };
            }

            export fn check_b(it: &Item) -> i32 {
                return it.y + it.z;
            }
        }

        // Top-level alias with the same local name
        export type Item = mod_b::Item;
    "#;
    fs::write(&defs_path, defs_src).unwrap();

    let defs_opts = CompilerOptions {
        output_path: Some(defs_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(defs_path.to_str().unwrap(), defs_src.to_string(), &defs_opts);
    assert!(res.is_ok(), "Compiling items_def.ln -> items_def.llib failed: {:?}", res.err());
    assert!(defs_llib.exists(), "items_def.llib must exist");

    // 2. Compile bridge.ln -> bridge.llib
    let bridge_path = dir.join("bridge.ln");
    let bridge_llib = dir.join("bridge.llib");
    let bridge_src = r#"
        import "items_def";

        export fn produce_a(val: i32) -> mod_a::Item {
            return mod_a::make_a(val);
        }

        export fn produce_b(val: i32) -> mod_b::Item {
            return mod_b::make_b(val);
        }
    "#;
    fs::write(&bridge_path, bridge_src).unwrap();

    let bridge_opts = CompilerOptions {
        output_path: Some(bridge_llib.to_str().unwrap().to_string()),
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(bridge_path.to_str().unwrap(), bridge_src.to_string(), &bridge_opts);
    assert!(res.is_ok(), "Compiling bridge.ln -> bridge.llib failed: {:?}", res.err());
    assert!(bridge_llib.exists(), "bridge.llib must exist");

    // 3. Compile main.ln consuming both .llib providers
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "items_def";
        import "bridge";

        fn main() -> i32 {
            dec item_a = produce_a(42);
            dec item_b = produce_b(10);

            // If identity collapsed to "Item", item_a would resolve to mod_b::Item (or alias Item),
            // which has fields y and z, but no field x, and typechecking check_a(&item_a) would fail.
            dec val_a = mod_a::check_a(&item_a);
            dec val_b = mod_b::check_b(&item_b);

            if item_a.x != 42 || val_a != 42 {
                return 1;
            }
            if item_b.y != 10 || item_b.z != 20 || val_b != 30 {
                return 2;
            }
            return 0;
        }
    "#;
    fs::write(&main_path, main_src).unwrap();

    let main_opts = CompilerOptions {
        output_path: Some(main_exe.to_str().unwrap().to_string()),
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), main_src.to_string(), &main_opts);
    assert!(res.is_ok(), "Compiling main.ln linking items_def and bridge failed: {:?}", res.err());
    assert!(main_exe.exists(), "main.exe must exist");

    // 4. Run binary and verify correct method execution
    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(
        code, 0,
        "Execution failed with code {}. Stderr: {}\nStdout: {}",
        code, stderr, stdout
    );
}

/// Scenario: Distinct providers `prov_x` and `prov_y` each declare `export struct Item`.
/// A bridge provider imports `prov_x` and returns `Item`.
/// The consumer imports `prov_x`, `prov_y`, and `bridge`.
/// Invariant: `prov_x::Item` and `prov_y::Item` are disambiguated by their `provider_name`
/// in `StableSymbolId`, so `prov_x::Item` is resolved accurately.
#[test]
fn test_cross_provider_same_local_name_disambiguation() {
    let dir = create_temp_dir("cross_provider_same_name");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // 1. prov_x.ln -> prov_x.llib
    let x_path = dir.join("prov_x.ln");
    let x_llib = dir.join("prov_x.llib");
    let x_src = r#"
        module mod_x {
            export struct Item {
                x_val: i32,
            };

            export fn make_x(v: i32) -> Item {
                return Item { x_val: v };
            }

            export fn get_x(it: Item) -> i32 {
                return it.x_val;
            }
        }
    "#;
    fs::write(&x_path, x_src).unwrap();

    let x_opts = CompilerOptions {
        output_path: Some(x_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(x_path.to_str().unwrap(), x_src.to_string(), &x_opts);
    assert!(res.is_ok(), "Compiling prov_x failed: {:?}", res.err());

    // 2. prov_y.ln -> prov_y.llib
    let y_path = dir.join("prov_y.ln");
    let y_llib = dir.join("prov_y.llib");
    let y_src = r#"
        module mod_y {
            export struct Item {
                y_val: i32,
            };

            export fn make_y(v: i32) -> Item {
                return Item { y_val: v };
            }

            export fn get_y(it: Item) -> i32 {
                return it.y_val;
            }
        }
    "#;
    fs::write(&y_path, y_src).unwrap();

    let y_opts = CompilerOptions {
        output_path: Some(y_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(y_path.to_str().unwrap(), y_src.to_string(), &y_opts);
    assert!(res.is_ok(), "Compiling prov_y failed: {:?}", res.err());

    // 3. bridge_x.ln -> bridge_x.llib
    let bridge_path = dir.join("bridge_x.ln");
    let bridge_llib = dir.join("bridge_x.llib");
    let bridge_src = r#"
        import "prov_x";

        export fn get_x_item() -> mod_x::Item {
            return mod_x::make_x(777);
        }
    "#;
    fs::write(&bridge_path, bridge_src).unwrap();

    let bridge_opts = CompilerOptions {
        output_path: Some(bridge_llib.to_str().unwrap().to_string()),
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(bridge_path.to_str().unwrap(), bridge_src.to_string(), &bridge_opts);
    assert!(res.is_ok(), "Compiling bridge_x failed: {:?}", res.err());

    // 4. main.ln imports prov_x, prov_y, and bridge_x
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "prov_x";
        import "prov_y";
        import "bridge_x";

        fn main() -> i32 {
            dec it = get_x_item();
            if mod_x::get_x(it) != 777 {
                return 1;
            }
            return 0;
        }
    "#;
    fs::write(&main_path, main_src).unwrap();

    let main_opts = CompilerOptions {
        output_path: Some(main_exe.to_str().unwrap().to_string()),
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), main_src.to_string(), &main_opts);
    assert!(res.is_ok(), "Compiling main failed: {:?}", res.err());

    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(
        code, 0,
        "Execution failed with code {}. Stderr: {}\nStdout: {}",
        code, stderr, stdout
    );
}
