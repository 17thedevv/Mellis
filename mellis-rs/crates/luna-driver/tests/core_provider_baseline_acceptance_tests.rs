use luna_driver::sysroot::Sysroot;
use luna_driver::sysroot_manifest::SysrootManifest;
use luna_driver::{check, compile, CompilerOptions};
use luna_llib::MlibReader;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_core_component_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn canonical_sysroot() -> Sysroot {
    Sysroot::discover_for_test().expect("Failed to locate canonical test sysroot")
}

fn canonical_manifest() -> SysrootManifest {
    let sysroot = canonical_sysroot();
    SysrootManifest::load_and_validate(&sysroot.external_dir().join("sysroot.toml"))
        .expect("canonical sysroot manifest must parse")
}

fn check_source(test_name: &str, source: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = canonical_sysroot();
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

#[test]
fn canonical_manifest_replaces_monolithic_core_provider() {
    let manifest = canonical_manifest();
    assert!(manifest.find_provider("core").is_none());
    for name in [
        "result",
        "ptr",
        "slice",
        "mem",
        "cmp",
        "hash",
        "clone",
        "copy",
        "iter_adapters",
        "iter_consumers",
        "try",
    ] {
        assert!(manifest.find_provider(name).is_some(), "missing logical provider {name}");
    }
    for contract in ["drop", "option", "iterator", "into_iterator"] {
        assert!(
            manifest.find_by_lang_contract(contract).is_some(),
            "missing language contract {contract}"
        );
    }
}

#[test]
fn component_sources_do_not_create_provider_namespaces() {
    let sysroot = canonical_sysroot();
    let manifest = canonical_manifest();
    for provider in manifest.providers() {
        if !provider.path.starts_with("core/") && !provider.path.starts_with("lang/") {
            continue;
        }
        let path = sysroot.external_dir().join(format!("{}.ln", provider.path));
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            !source.contains("core::"),
            "provider path must not synthesize a core namespace: {}",
            path.display()
        );
    }
}

#[test]
fn language_contracts_are_auto_loaded_for_user_types() {
    let source = r#"
        struct UserIter { current: i32 }

        impl Iterator<i32> for UserIter {
            fn next(self: &rw Self) -> Option<i32> {
                if self.current == 0 {
                    self.current = 1;
                    return Option::Some(7);
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec rw iter = UserIter { current: 0 };
            match iter.next() {
                Option::Some(value) -> { return value - 7; },
                Option::None -> { return 1; },
            }
        }
    "#;
    assert!(check_source("auto_loaded_contracts", source).is_ok());
}

#[test]
fn ordinary_core_api_remains_explicit() {
    let without_import = "fn consume(value: Result<i32, i32>) {}";
    assert!(check_source("result_without_import", without_import).is_err());

    let with_import = r#"
        import <result>;
        fn consume(value: Result<i32, i32>) {}
        fn main() -> i32 { return 0; }
    "#;
    assert!(check_source("result_with_import", with_import).is_ok());
}

#[test]
fn internal_language_provider_rejects_user_import() {
    let source = "import <__lang_drop>; fn main() -> i32 { return 0; }";
    assert!(check_source("internal_lang_user_import", source).is_err());
}

#[test]
fn result_component_survives_the_full_pipeline() {
    let sysroot = canonical_sysroot();
    let dir = create_temp_dir("result_full_pipeline");
    let path = dir.join("main.ln");
    let source = r#"
        import <result>;

        fn pass(value: Result<i32, i32>) -> Result<i32, i32> {
            return value;
        }

        fn main() -> i32 {
            dec value = pass(Result::Ok(7));
            return 0;
        }
    "#;
    fs::write(&path, source).unwrap();
    let output = dir.join("main.obj");
    let options = CompilerOptions {
        output_path: Some(output.to_string_lossy().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        emit_llvm: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let result = compile(path.to_str().unwrap(), source.to_string(), &options);
    assert!(result.is_ok(), "result component must compile: {:?}", result.err());
    assert!(output.exists());
    assert!(dir.join("main.ll").exists());
}

#[test]
fn component_artifacts_are_present_and_readable() {
    let sysroot = canonical_sysroot();
    let manifest = canonical_manifest();
    for provider in manifest.providers() {
        if !provider.path.starts_with("core/") && !provider.path.starts_with("lang/") {
            continue;
        }
        let source_path = sysroot.external_dir().join(format!("{}.ln", provider.path));
        let artifact_path = sysroot.external_dir().join(format!("{}.llib", provider.path));
        assert!(source_path.exists(), "missing source {}", source_path.display());
        let mut file = fs::File::open(&artifact_path)
            .unwrap_or_else(|error| panic!("missing artifact {}: {error}", artifact_path.display()));
        MlibReader::read_module(&mut file)
            .unwrap_or_else(|error| panic!("invalid artifact {}: {error:?}", artifact_path.display()));
    }
}


