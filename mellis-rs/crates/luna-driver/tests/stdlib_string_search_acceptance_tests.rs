use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_string_search_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_and_run(test_name: &str, source: &str) -> (i32, String, String) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    let exe_path = temp.join(format!("{}.exe", test_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), source.to_string(), &options);
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

#[test]
fn test_search_contains_and_find() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s = string_from_str("hello world, hello luna!");

    // 1. contains
    if s.contains("world") == false {
        return 1;
    }
    if s.contains("planet") {
        return 2;
    }
    if s.contains("") == false {
        return 3;
    }

    // 2. contains_char
    if s.contains_char('w') == false {
        return 4;
    }
    if s.contains_char('z') {
        return 5;
    }

    // 3. find
    match s.find("world") {
        Option::Some(pos) -> {
            if pos != (6 as u64) {
                return 6;
            }
        },
        Option::None -> {
            return 7;
        },
    };

    match s.find("missing") {
        Option::Some(_) -> {
            return 8;
        },
        Option::None -> {},
    };

    match s.find("") {
        Option::Some(pos) -> {
            if pos != (0 as u64) {
                return 9;
            }
        },
        Option::None -> {
            return 10;
        },
    };

    // 4. find_char
    match s.find_char(',') {
        Option::Some(pos) -> {
            if pos != (11 as u64) {
                return 11;
            }
        },
        Option::None -> {
            return 12;
        },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_search_contains_and_find", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_search_utf8_byte_offsets() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // "Xin chào thế giới!"
    // 'à' is 2 bytes (0xC3 0xA0), 'ế' is 2 bytes (0xE1 0xBA 0xBF), 'ớ' is 2 bytes (0xE1 0xBB 0x9B)
    dec s = string_from_str("Xin chào thế giới!");

    // "Xin " is 4 bytes: 0, 1, 2, 3 -> "chào" starts at byte 4
    match s.find("chào") {
        Option::Some(pos) -> {
            if pos != (4 as u64) {
                return 1;
            }
        },
        Option::None -> {
            return 2;
        },
    };

    if s.contains("thế") == false {
        return 3;
    }
    if s.contains("giới") == false {
        return 4;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_search_utf8_byte_offsets", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_search_starts_and_ends_with() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s = string_from_str("mellis_compiler_luna");

    if s.starts_with("mellis") == false {
        return 1;
    }
    if s.starts_with("luna") {
        return 2;
    }
    if s.starts_with("") == false {
        return 3;
    }

    if s.ends_with("luna") == false {
        return 4;
    }
    if s.ends_with("mellis") {
        return 5;
    }
    if s.ends_with("") == false {
        return 6;
    }

    // prefix longer than string
    if s.starts_with("mellis_compiler_luna_extra_long") {
        return 7;
    }
    if s.ends_with("mellis_compiler_luna_extra_long") {
        return 8;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_search_starts_and_ends_with", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_search_push_string() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec rw s1 = string_from_str("hello ");
    dec s2 = string_from_str("world!");
    s1.push_string(&s2);

    if s1.len() != (12 as u64) {
        return 1;
    }
    if s1.ends_with("world!") == false {
        return 2;
    }
    if s1.starts_with("hello ") == false {
        return 3;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_search_push_string", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}
