use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_text_parity_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

const TEXT_PROVIDER: &str = r#"
import <string>;
import <vec>;

module text_tools {
    export fn count_tokens(s: &String, delim: char) -> u64 {
        dec parts = s.split(delim);
        return parts.len();
    }

    export fn parse_and_double(s: &String) -> i32 {
        match s.parse_i32() {
            Option::Some(v) -> {
                return v * 2;
            },
            Option::None -> {
                return (0 as i32) - (1 as i32);
            },
        }
    }

    export fn format_msg(val: i64) -> String {
        return string_from_i64(val);
    }
}
"#;

const TEXT_CONSUMER: &str = r#"
import "text_tools";
import <string>;

fn main() -> i32 {
    dec s = string_from_str("one,two,three");
    dec cnt = text_tools::count_tokens(&s, ',');
    if cnt != (3 as u64) {
        return 1;
    }

    dec num_s = string_from_str("21");
    dec doubled = text_tools::parse_and_double(&num_s);
    if doubled != 42 {
        return 2;
    }

    dec msg = text_tools::format_msg(999 as i64);
    if msg.eq_str("999") == false {
        return 3;
    }

    return 0;
}
"#;

fn run_consumer(dir: &Path, sysroot: &str) -> (bool, i32) {
    let main_path = dir.join("main.ln");
    let exe = dir.join("main.exe");
    let _ = fs::remove_file(&exe);
    fs::write(&main_path, TEXT_CONSUMER).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe.to_str().unwrap().to_string()),
        search_paths: vec![dir.to_str().unwrap().to_string(), sysroot.to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(main_path.to_str().unwrap(), TEXT_CONSUMER.to_string(), &options);
    if res.is_err() {
        return (false, -1);
    }

    let out = Command::new(&exe).output().expect("failed to execute consumer");
    (true, out.status.code().unwrap_or(-1))
}

#[test]
fn test_text_provider_source_and_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot not found");
    let sysroot_root = sysroot.root().to_str().unwrap().to_string();

    let dir = temp_dir("parity_text");
    let prov_ln = dir.join("text_tools.ln");
    let prov_llib = dir.join("text_tools.llib");

    // Phase 1: Compile provider source
    fs::write(&prov_ln, TEXT_PROVIDER).unwrap();

    // Verify consumer compiles and runs against source provider
    let (src_ok, src_exit) = run_consumer(&dir, &sysroot_root);
    assert!(src_ok, "consumer compilation against source provider failed");
    assert_eq!(src_exit, 0, "consumer against source provider exited with code {}", src_exit);

    // Phase 2: Compile provider to .llib
    let lib_opts = CompilerOptions {
        output_path: Some(prov_llib.to_str().unwrap().to_string()),
        search_paths: vec![sysroot_root.clone()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let lib_res = compile(prov_ln.to_str().unwrap(), TEXT_PROVIDER.to_string(), &lib_opts);
    assert!(lib_res.is_ok(), "failed to compile provider to .llib: {:?}", lib_res.err());
    assert!(prov_llib.is_file(), "provider .llib artifact was not created");

    // Phase 3: Delete provider source, consumer must now resolve from .llib
    fs::remove_file(&prov_ln).unwrap();
    assert!(!prov_ln.exists(), "provider source should be removed");

    let (llib_ok, llib_exit) = run_consumer(&dir, &sysroot_root);
    assert!(llib_ok, "consumer compilation against .llib artifact failed");
    assert_eq!(llib_exit, 0, "consumer against .llib artifact exited with code {}", llib_exit);
}
