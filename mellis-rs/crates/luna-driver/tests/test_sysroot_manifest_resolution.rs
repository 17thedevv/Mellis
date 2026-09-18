use std::fs;
use std::path::PathBuf;
use std::process::Command;
use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use luna_driver::discovery::ExternalComponentDiscovery;
use luna_driver::resolution_context::ProviderResolutionContext;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("luna_sysroot_manifest_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_discovery_rejects_internal_provider_in_user_context() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let manifest = sysroot.manifest();
    let ext_dir = sysroot.external_dir();

    // __alloc_global is internal
    let res_global_user = ExternalComponentDiscovery::discover(
        ext_dir,
        "__alloc_global",
        manifest,
        ProviderResolutionContext::UserImport,
    );
    assert!(res_global_user.is_err(), "User context must not discover internal __alloc_global");

    // __raw_table is internal
    let res_raw_user = ExternalComponentDiscovery::discover(
        ext_dir,
        "__raw_table",
        manifest,
        ProviderResolutionContext::UserImport,
    );
    assert!(res_raw_user.is_err(), "User context must not discover internal __raw_table");

    // SysrootDependency context can discover internal providers
    let res_global_sys = ExternalComponentDiscovery::discover(
        ext_dir,
        "__alloc_global",
        manifest,
        ProviderResolutionContext::SysrootDependency,
    );
    assert!(res_global_sys.is_ok(), "SysrootDependency context must discover internal __alloc_global: {:?}", res_global_sys.err());

    let res_raw_sys = ExternalComponentDiscovery::discover(
        ext_dir,
        "__raw_table",
        manifest,
        ProviderResolutionContext::SysrootDependency,
    );
    assert!(res_raw_sys.is_ok(), "SysrootDependency context must discover internal __raw_table: {:?}", res_raw_sys.err());
}

#[test]
fn test_user_cannot_directly_import_alloc_global() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("user_import_alloc_global");
    let src_path = dir.join("main.ln");

    let src = r#"
        import <__alloc_global>;

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        is_sysroot_build: false,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Direct user import of internal provider __alloc_global must fail");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("__alloc_global")),
        "Diagnostic must mention __alloc_global: {:?}",
        diags
    );
}

#[test]
fn test_user_cannot_directly_import_raw_table() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("user_import_raw_table");
    let src_path = dir.join("main.ln");

    let src = r#"
        import <__raw_table>;

        fn main() -> i32 {
            return 0;
        }
    "#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        is_sysroot_build: false,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Direct user import of internal provider __raw_table must fail");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("__raw_table")),
        "Diagnostic must mention __raw_table: {:?}",
        diags
    );
}

#[test]
fn test_transitive_access_to_internal_providers_succeeds() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("transitive_internal_access");
    let src_path = dir.join("main.ln");
    let exe_path = dir.join(if cfg!(windows) { "main.exe" } else { "main" });

    // User imports public providers <box>, <vec>, <hashmap> which internally use __alloc_global and __raw_table
    let src = r#"
        import <box>;
        import <vec>;
        import <hashmap>;

        fn main() -> i32 {
            dec b = std::box_new<i32>(42);
            dec rw v = vec_new<i32>();
            v.push(std::box_into_inner(b));

            dec rw map = hashmap_with_capacity<i32, i32>(4 as u64);
            map.insert(1, 42);

            dec val = map.get(&1);
            match val {
                Option::Some(x) -> {
                    if *x == 42 {
                        return 0;
                    }
                },
                Option::None -> {},
            }
            return 1;
        }
    "#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        is_sysroot_build: false,
        ..Default::default()
    };

    let compile_res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(compile_res.is_ok(), "User code importing public providers must compile cleanly: {:?}", compile_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Execution must succeed");
    assert_eq!(output.status.code().unwrap_or(-1), 0, "Binary must execute with return code 0");
}

#[test]
fn test_user_cannot_directly_import_internal_lang_contracts() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let contracts = [
        "__lang_drop",
        "__lang_option",
        "__lang_iterator",
        "__lang_into_iterator",
    ];

    for contract in contracts {
        let dir = create_temp_dir(&format!("user_import_{}", contract));
        let src_path = dir.join("main.ln");
        let src = format!("import <{}>;\nfn main() -> i32 {{ return 0; }}\n", contract);
        fs::write(&src_path, &src).unwrap();

        let opts = CompilerOptions {
            search_paths: vec![sysroot.root().to_string_lossy().to_string()],
            quiet: true,
            is_sysroot_build: false,
            ..Default::default()
        };

        let res = check(src_path.to_str().unwrap(), src, &opts);
        assert!(
            res.is_err(),
            "Direct user import of internal provider {} must fail",
            contract
        );
        let diags = res.err().unwrap();
        assert!(
            diags.iter().any(|d| d.message.contains(contract)),
            "Diagnostic must mention {}: {:?}",
            contract,
            diags
        );
    }
}

#[test]
fn test_same_session_cached_internal_provenance_regression() {
    use luna_common::CompilerSession;
    use luna_driver::session::DriverSession;

    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let mut compiler_session = CompilerSession::new();
    let mut driver_session = DriverSession::new(sysroot, &mut compiler_session, &[]);
    let mut arena = luna_ast::AstArena::new();

    // 1. Bootstrap language contracts into this session.
    // This loads __lang_drop, __lang_option, __lang_iterator, __lang_into_iterator into self.registry.providers!
    driver_session
        .bootstrap_lang_contracts(&mut arena)
        .expect("language contract bootstrap must succeed");

    // Verify __lang_option and __lang_drop are indeed cached in registry
    assert!(driver_session.registry.providers.contains_key("__lang_option"));
    assert!(driver_session.registry.providers.contains_key("__lang_drop"));

    // Now, in the SAME DriverSession, attempt to load __lang_option as UserImport
    let err_opt = driver_session.load_package(
        "__lang_option",
        &mut arena,
        ProviderResolutionContext::UserImport,
    );
    assert!(
        err_opt.is_err(),
        "Direct UserImport of cached internal __lang_option must be rejected"
    );

    let err_drop = driver_session.load_package(
        "__lang_drop",
        &mut arena,
        ProviderResolutionContext::UserImport,
    );
    assert!(
        err_drop.is_err(),
        "Direct UserImport of cached internal __lang_drop must be rejected"
    );

    // 2. Load public provider "hashmap" as UserImport.
    // This transitively loads "__raw_table" and "__alloc_global" via SysrootDependency!
    let hashmap_res = driver_session.load_package(
        "hashmap",
        &mut arena,
        ProviderResolutionContext::UserImport,
    );
    assert!(hashmap_res.is_ok(), "hashmap must load successfully: {:?}", hashmap_res.err());

    // Verify __raw_table and __alloc_global are now cached in the registry
    assert!(driver_session.registry.providers.contains_key("__raw_table"));
    assert!(driver_session.registry.providers.contains_key("__alloc_global"));

    // In the SAME DriverSession, attempt direct UserImport of cached __raw_table and __alloc_global
    let err_raw = driver_session.load_package(
        "__raw_table",
        &mut arena,
        ProviderResolutionContext::UserImport,
    );
    assert!(
        err_raw.is_err(),
        "Direct UserImport of cached internal __raw_table must be rejected"
    );

    let err_global = driver_session.load_package(
        "__alloc_global",
        &mut arena,
        ProviderResolutionContext::UserImport,
    );
    assert!(
        err_global.is_err(),
        "Direct UserImport of cached internal __alloc_global must be rejected"
    );
}

