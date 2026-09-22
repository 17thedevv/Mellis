use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_audit_stdlib_gap_probes() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let probes_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/stdlib_gap_probes");

    let probe_names = vec![
        "console_hello",
        "console_read_line",
        "cli_args",
        "env_read",
        "file_read_all",
        "file_write",
        "file_copy",
        "text_split",
        "text_find",
        "parse_integer",
        "format_values",
        "word_frequency",
        "path_join",
        "vec_transform",
        "hashmap_count",
        "result_error_flow",
        "simple_config_parse",
    ];

    let temp_dir = std::env::temp_dir().join("luna_stdlib_gap_audit");
    let _ = fs::create_dir_all(&temp_dir);

    println!("\n=== RUNNING STDLIB GAP AUDIT PROBES ===");

    for name in &probe_names {
        let ln_path = probes_dir.join(format!("{}.ln", name));
        assert!(ln_path.exists(), "Probe file missing: {:?}", ln_path);
        let source = fs::read_to_string(&ln_path).unwrap();

        let exe_path = temp_dir.join(format!("{}.exe", name));
        let options = CompilerOptions {
            output_path: Some(exe_path.to_str().unwrap().to_string()),
            search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
            quiet: true,
            ..Default::default()
        };

        match compile(ln_path.to_str().unwrap(), source, &options) {
            Ok(_) => {
                println!("PROBE [{}]: COMPILED_SUCCESSFULLY", name);
            }
            Err(diags) => {
                let msgs: Vec<String> = diags.iter().map(|d| d.message.clone()).collect();
                println!("PROBE [{}]: FAILED (as expected/audited) -> {:?}", name, msgs);
            }
        }
    }
}
