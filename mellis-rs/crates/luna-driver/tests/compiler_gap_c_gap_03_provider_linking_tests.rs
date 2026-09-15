use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
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
    let dir = std::env::temp_dir().join(format!("luna_c_gap_03_{}_{}", name, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

// -----------------------------------------------------------------------------
// Test 1: Single provider non-generic function compiled to .llib, imported & linked
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_single_provider_non_generic_link_and_exec() {
    let dir = create_temp_dir("single_provider");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // 1. Provider math.ln -> math.llib
    let math_path = dir.join("math.ln");
    let math_llib = dir.join("math.llib");
    let math_src = r#"
        export fn add(a: i32, b: i32) -> i32 {
            return a + b;
        }
    "#;
    fs::write(&math_path, math_src).unwrap();

    let math_opts = CompilerOptions {
        output_path: Some(math_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(math_path.to_str().unwrap(), math_src.to_string(), &math_opts);
    assert!(res.is_ok(), "Compiling math.ln -> math.llib failed: {:?}", res.err());
    assert!(math_llib.exists(), "math.llib must exist");

    // 2. Consumer main.ln importing "math"
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "math";

        fn main() -> i32 {
            dec res = add(10, 32);
            if res == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking main.ln failed: {:?}", res.err());
    assert!(main_exe.exists(), "main.exe must exist");

    // 3. Execute native binary
    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 2: Provider embedded object code extraction (when sidecar .obj is removed)
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_provider_embedded_object_code_extraction() {
    let dir = create_temp_dir("embedded_obj");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // 1. Provider calc.ln -> calc.llib
    let calc_path = dir.join("calc.ln");
    let calc_llib = dir.join("calc.llib");
    let calc_obj = dir.join("calc.obj");
    let calc_src = r#"
        export fn multiply(a: i32, b: i32) -> i32 {
            return a * b;
        }
    "#;
    fs::write(&calc_path, calc_src).unwrap();

    let calc_opts = CompilerOptions {
        output_path: Some(calc_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(calc_path.to_str().unwrap(), calc_src.to_string(), &calc_opts);
    assert!(res.is_ok(), "Compiling calc.ln -> calc.llib failed: {:?}", res.err());
    assert!(calc_llib.exists(), "calc.llib must exist");

    // Deliberately delete sidecar .obj to force extraction from embedded SectionType::ObjectCode
    if calc_obj.exists() {
        fs::remove_file(&calc_obj).expect("Failed to remove sidecar .obj");
    }
    assert!(!calc_obj.exists(), "Sidecar .obj must be absent");

    // 2. Consumer main.ln importing "calc"
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "calc";

        fn main() -> i32 {
            dec res = multiply(6, 7);
            if res == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking main.ln with extracted .obj failed: {:?}", res.err());
    assert!(main_exe.exists(), "main.exe must exist");

    // 3. Execute native binary
    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 3: Multi-provider dependency chain (Consumer -> Provider B -> Provider A)
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_multi_provider_link_dependency_chain() {
    let dir = create_temp_dir("dep_chain");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // 1. Provider base.ln -> base.llib
    let base_path = dir.join("base.ln");
    let base_llib = dir.join("base.llib");
    let base_src = r#"
        export fn base_val() -> i32 {
            return 40;
        }
    "#;
    fs::write(&base_path, base_src).unwrap();

    let base_opts = CompilerOptions {
        output_path: Some(base_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(base_path.to_str().unwrap(), base_src.to_string(), &base_opts);
    assert!(res.is_ok(), "Compiling base.ln failed: {:?}", res.err());

    // 2. Provider mid.ln (imports base) -> mid.llib
    let mid_path = dir.join("mid.ln");
    let mid_llib = dir.join("mid.llib");
    let mid_src = r#"
        import "base";

        export fn mid_val() -> i32 {
            return base_val() + 2;
        }
    "#;
    fs::write(&mid_path, mid_src).unwrap();

    let mid_opts = CompilerOptions {
        output_path: Some(mid_llib.to_str().unwrap().to_string()),
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(mid_path.to_str().unwrap(), mid_src.to_string(), &mid_opts);
    assert!(res.is_ok(), "Compiling mid.ln failed: {:?}", res.err());

    // 3. Consumer main.ln (imports mid) -> main.exe
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "mid";

        fn main() -> i32 {
            dec res = mid_val();
            if res == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking chained dependencies failed: {:?}", res.err());

    // 4. Execute native binary
    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 4: Generic and non-generic mixed provider functions
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_generic_and_non_generic_mixed_provider() {
    let dir = create_temp_dir("mixed_provider");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    let utils_path = dir.join("utils.ln");
    let utils_llib = dir.join("utils.llib");
    let utils_src = r#"
        export fn get_multiplier() -> i32 {
            return 10;
        }

        export struct BoxedVal<T> {
            export val: T,
        }

        impl<T> BoxedVal<T> {
            export fn get(self: &BoxedVal<T>) -> T {
                return self.val;
            }
        }
    "#;
    fs::write(&utils_path, utils_src).unwrap();

    let utils_opts = CompilerOptions {
        output_path: Some(utils_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(utils_path.to_str().unwrap(), utils_src.to_string(), &utils_opts);
    assert!(res.is_ok(), "Compiling utils.ln failed: {:?}", res.err());

    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "utils";

        fn main() -> i32 {
            dec b = BoxedVal<i32> { val: 4 };
            dec m = get_multiplier();
            dec res = b.get() * m + 2;
            if res == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking mixed provider failed: {:?}", res.err());

    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 5: Provider custom Drop and state execution
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_custom_drop_and_provider_state() {
    let dir = create_temp_dir("provider_drop");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    let tracker_path = dir.join("tracker.ln");
    let tracker_llib = dir.join("tracker.llib");
    let tracker_src = r#"
        export struct ManagedResource {
            export status: i32,
        }

        impl Drop for ManagedResource {
            export fn drop(self: &rw ManagedResource) {
                self.status = 99;
            }
        }

        export fn create_resource(s: i32) -> ManagedResource {
            return ManagedResource { status: s };
        }
    "#;
    fs::write(&tracker_path, tracker_src).unwrap();

    let tracker_opts = CompilerOptions {
        output_path: Some(tracker_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(tracker_path.to_str().unwrap(), tracker_src.to_string(), &tracker_opts);
    assert!(res.is_ok(), "Compiling tracker.ln failed: {:?}", res.err());

    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import <core>;
        import "tracker";

        fn main() -> i32 {
            dec r = create_resource(42);
            if r.status == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking provider with Drop failed: {:?}", res.err());

    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 6: Core & Alloc sysroot binary E2E native execution
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_core_alloc_sysroot_binary_e2e() {
    let dir = create_temp_dir("core_alloc_e2e");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec b = std::box_new(42);
            dec v = b.as_ref();
            if *v == 42 {
                return 0;
            }
            return 1;
        }
    "#;
    fs::write(&main_path, main_src).unwrap();

    let main_opts = CompilerOptions {
        output_path: Some(main_exe.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), main_src.to_string(), &main_opts);
    assert!(res.is_ok(), "Compiling & linking core + alloc failed: {:?}", res.err());

    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}

// -----------------------------------------------------------------------------
// Test 7: Cross-module symbol isolation across multiple providers
// -----------------------------------------------------------------------------
#[test]
fn test_c_gap_03_cross_module_symbol_isolation() {
    let dir = create_temp_dir("sym_isolation");
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");

    // Provider mod_a
    let mod_a_path = dir.join("mod_a.ln");
    let mod_a_llib = dir.join("mod_a.llib");
    let mod_a_src = r#"
        module alpha {
            export fn calculate() -> i32 {
                return 20;
            }
        }
    "#;
    fs::write(&mod_a_path, mod_a_src).unwrap();

    let mod_a_opts = CompilerOptions {
        output_path: Some(mod_a_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(mod_a_path.to_str().unwrap(), mod_a_src.to_string(), &mod_a_opts);
    assert!(res.is_ok(), "Compiling mod_a.ln failed: {:?}", res.err());

    // Provider mod_b
    let mod_b_path = dir.join("mod_b.ln");
    let mod_b_llib = dir.join("mod_b.llib");
    let mod_b_src = r#"
        module beta {
            export fn calculate() -> i32 {
                return 22;
            }
        }
    "#;
    fs::write(&mod_b_path, mod_b_src).unwrap();

    let mod_b_opts = CompilerOptions {
        output_path: Some(mod_b_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(mod_b_path.to_str().unwrap(), mod_b_src.to_string(), &mod_b_opts);
    assert!(res.is_ok(), "Compiling mod_b.ln failed: {:?}", res.err());

    // Consumer main.ln
    let main_path = dir.join("main.ln");
    let main_exe = dir.join("main.exe");
    let main_src = r#"
        import "mod_a";
        import "mod_b";

        fn main() -> i32 {
            dec a = alpha::calculate();
            dec b = beta::calculate();
            if a + b == 42 {
                return 0;
            }
            return 1;
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
    assert!(res.is_ok(), "Compiling & linking isolated module symbols failed: {:?}", res.err());

    let (code, stdout, stderr) = run_executable(&main_exe);
    assert_eq!(code, 0, "Execution failed with code {}. Stderr: {}\nStdout: {}", code, stderr, stdout);
}
