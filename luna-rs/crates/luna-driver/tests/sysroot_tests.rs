use luna_ast::AstArena;
use luna_common::CompilerSession;
use luna_driver::resolution_context::ProviderResolutionContext;
use luna_driver::sysroot::Sysroot;
use luna_driver::{check, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_sysroot_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_manifest(root: &std::path::Path, body: &str) {
    let ext = root.join("libs").join("external");
    fs::create_dir_all(&ext).unwrap();
    fs::write(ext.join("sysroot.toml"), body).unwrap();
}

#[test]
fn test_sysroot_missing_dir_diagnostic() {
    let non_existent = PathBuf::from("C:/definitely_does_not_exist_mellis_sysroot_12345");
    let res = Sysroot::discover(Some(non_existent.to_str().unwrap()));
    assert!(res.is_err(), "Expected error for nonexistent sysroot path");
    let diags = res.err().unwrap().into_diagnostics();
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0].message.contains("does not exist"),
        "Unexpected message: {}",
        diags[0].message
    );
}

#[test]
fn test_sysroot_missing_language_contract_diagnostic() {
    let temp = create_temp_dir("missing_language_contracts");
    write_manifest(&temp, "provider = []\n");

    let dummy_source = "fn main() -> i32 { return 0; }".to_string();
    let dummy_path = temp.join("main.ln");
    fs::write(&dummy_path, &dummy_source).unwrap();

    let options = CompilerOptions {
        search_paths: vec![temp.to_str().unwrap().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(dummy_path.to_str().unwrap(), dummy_source, &options);
    assert!(
        res.is_err(),
        "Expected error when language-contract providers are missing"
    );
    let diags = res.err().unwrap();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("contract") || d.message.contains("drop")),
        "Expected diagnostic mentioning a missing language contract, got: {:?}",
        diags
    );
}

static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_sysroot_env_var_override() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let temp = create_temp_dir("env_override");
    let ext_dir = temp.join("libs").join("external");
    write_manifest(&temp, "provider = []\n");

    std::env::set_var("LUNA_SYSROOT", temp.to_str().unwrap());
    let res = Sysroot::discover(None);
    std::env::remove_var("LUNA_SYSROOT");

    assert!(
        res.is_ok(),
        "Expected discovery via LUNA_SYSROOT to succeed"
    );
    let sysroot = res.unwrap();
    assert_eq!(sysroot.root(), temp.as_path());
    assert_eq!(sysroot.external_dir(), ext_dir.as_path());
}

#[test]
fn test_sysroot_explicit_cli_override() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let env_dir = create_temp_dir("env_dir");
    let cli_dir = create_temp_dir("cli_dir");
    write_manifest(&env_dir, "provider = []\n");
    write_manifest(&cli_dir, "provider = []\n");

    std::env::set_var("LUNA_SYSROOT", env_dir.to_str().unwrap());
    let res = Sysroot::discover(Some(cli_dir.to_str().unwrap()));
    std::env::remove_var("LUNA_SYSROOT");

    assert!(res.is_ok());
    let sysroot = res.unwrap();
    assert_eq!(sysroot.root(), cli_dir.as_path());
}

#[test]
fn test_core_loaded_only_once() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let mut session = CompilerSession::new();
    let mut driver_session = luna_driver::DriverSession::new(sysroot, &mut session, &[]);

    let mut arena = AstArena::new();
    let source = String::new();

    // 1. Bootstrap core
    let core_id1 = driver_session
        .load_package(
            "__lang_drop",
            &mut arena,
            ProviderResolutionContext::SysrootDependency,
        )
        .expect("Failed to load __lang_drop");

    let decl_count_after_bootstrap = arena.decls.len();
    assert!(
        decl_count_after_bootstrap > 0,
        "Core should introduce AST declarations"
    );

    // 2. Request core via load_package
    let core_id2 = driver_session
        .load_package(
            "__lang_drop",
            &mut arena,
            ProviderResolutionContext::SysrootDependency,
        )
        .expect("load_package('core') should succeed");

    assert_eq!(core_id1, core_id2, "ProviderId must be identical");
    assert_eq!(
        arena.decls.len(),
        decl_count_after_bootstrap,
        "AST declarations must not be duplicated on second load"
    );
}

#[test]
fn test_import_core_reuses_bootstrapped_component() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("import_core");
    let main_path = dir.join("main.ln");
    let main_src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&main_path, main_src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(main_path.to_str().unwrap(), main_src.to_string(), &options);
    assert!(
        res.is_ok(),
        "Compiling with canonical component imports must succeed and reuse bootstrapped providers, got: {:?}",
        res.err()
    );
}

#[test]
fn test_external_component_identity_stable() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let mut session = CompilerSession::new();
    let mut driver_session = luna_driver::DriverSession::new(sysroot, &mut session, &[]);

    let mut arena = AstArena::new();
    let source = String::new();

    let core_id = driver_session
        .load_package(
            "__lang_drop",
            &mut arena,
            ProviderResolutionContext::SysrootDependency,
        )
        .expect("Failed to load __lang_drop");

    let interface = driver_session
        .registry
        .interfaces
        .get(&core_id)
        .expect("Interface must exist in registry");

    assert_eq!(interface.name, "__lang_drop");
    assert_eq!(interface.id, core_id);

    // Verify Drop symbol exists and has provider_id == core_id
    let drop_sym = interface
        .exported_symbols
        .get("Drop")
        .expect("Drop contract provider must export Drop");
    assert_eq!(drop_sym.sym.provider_id, Some(core_id));
}
