use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_binary(exe_path: &Path) -> (i32, String, String) {
    let output = Command::new(exe_path)
        .output()
        .expect("Failed to execute produced binary");
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (exit_code, stdout, stderr)
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_slice_iter_tests_{}", name));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 1. Positive slice iterator next_ref borrowed projection
#[test]
fn test_slice_iterator_read() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_read");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

    let src = r#"
import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    v.push(30);

    dec s = v.as_slice();
    dec rw it = s.iter();

    dec v1 = match it.next_ref() {
        Option::Some(r) -> *r,
        Option::None -> 0,
    };
    dec v2 = match it.next_ref() {
        Option::Some(r) -> *r,
        Option::None -> 0,
    };
    dec v3 = match it.next_ref() {
        Option::Some(r) -> *r,
        Option::None -> 0,
    };
    dec v4_is_none = match it.next_ref() {
        Option::Some(_) -> false,
        Option::None -> true,
    };

    if v1 == 10 && v2 == 20 && v3 == 30 && v4_is_none {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Slice iterator read MUST compile: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Binary must exit with 0 (all elements iterated correctly)");
}

/// 2. Positive slice iterator next() value yield
#[test]
fn test_slice_iterator_trait_next() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_trait_next");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

    let src = r#"
import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(100);
    v.push(200);

    dec s = v.as_slice();
    dec rw it = s.iter();

    dec item1 = it.next().unwrap();
    dec item2 = it.next().unwrap();
    dec item3_is_none = it.next().is_none();

    if item1 == 100 && item2 == 200 && item3_is_none {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Slice iterator trait next MUST compile: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Binary must exit with 0 (all elements yielded by value)");
}

/// 3. Positive slice iterator next_mut mutation
#[test]
fn test_slice_iterator_mut_write() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_mut_write");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

    let src = r#"
import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);

    dec rw s = v.as_mut_slice();
    dec rw it = s.iter_mut();

    match it.next_mut() {
        Option::Some(r) -> {
            *r = 15;
        }
        Option::None -> {}
    }
    match it.next_mut() {
        Option::Some(r) -> {
            *r = 25;
        }
        Option::None -> {}
    }

    dec res_s = v.as_slice();
    if res_s[0] == 15 && res_s[1] == 25 {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Slice iterator mut write MUST compile: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Binary must exit with 0 (mutated slice elements reflect in Vec)");
}

/// 4. Borrowck rejects mutating container while iterator is live
#[test]
fn test_slice_iterator_borrow_conflict() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("borrow_conflict");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_new<i32>();
    v.push(10);
    dec s = v.as_slice();
    dec rw it = s.iter();
    v.push(20); // conflict: v is borrowed while iterator is alive
    dec n = it.next_ref();
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Mutating vector while slice iterator is alive MUST be rejected by borrowck!");
}

/// 5. Borrowck allows mutating container after iterator is dropped
#[test]
fn test_slice_iterator_borrow_after_drop_allowed() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("borrow_after_drop");
    let src_path = dir.join("main.ln");

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_new<i32>();
    v.push(10);
    {
        dec s = v.as_slice();
        dec rw it = s.iter();
        dec n = it.next_ref();
    }
    v.push(20); // allowed: iterator and slice have dropped
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Mutating vector after slice iterator has dropped MUST be allowed: {:?}", res.err());
}

/// 6. Provider .llib export and consumer import parity
#[test]
fn test_slice_iterator_source_and_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_parity");

    let lib_src_path = dir.join("iter_helper.ln");
    let lib_bin_path = dir.join("iter_helper.llib");
    let consumer_path = dir.join("consumer.ln");

    let lib_src = r#"
import <core>;

export fn count_positive(s: &[i32]) -> u64 {
    dec rw it = s.iter();
    dec rw count: u64 = 0 as u64;
    while true {
        match it.next_ref() {
            Option::Some(r) -> {
                if *r > 0 {
                    count = count + (1 as u64);
                }
            }
            Option::None -> {
                break;
            }
        }
    }
    return count;
}
"#;
    fs::write(&lib_src_path, lib_src).unwrap();

    let compile_opts = CompilerOptions {
        output_path: Some(lib_bin_path.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };
    let res_compile = compile(lib_src_path.to_str().unwrap(), lib_src.to_string(), &compile_opts);
    assert!(res_compile.is_ok(), "Compiling iter_helper.ln to .llib MUST succeed: {:?}", res_compile.err());

    let consumer_src = r#"
import <core>;
import <alloc>;
import "iter_helper";

fn main() {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(-5);
    v.push(20);
    dec s = v.as_slice();
    dec c = count_positive(s);
}
"#;
    fs::write(&consumer_path, consumer_src).unwrap();

    let consumer_opts = CompilerOptions {
        no_link: true,
        search_paths: vec![
            dir.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        quiet: true,
        ..Default::default()
    };

    let res_consumer = compile(consumer_path.to_str().unwrap(), consumer_src.to_string(), &consumer_opts);
    assert!(res_consumer.is_ok(), "Consumer using .llib slice iterator MUST compile: {:?}", res_consumer.err());
}
