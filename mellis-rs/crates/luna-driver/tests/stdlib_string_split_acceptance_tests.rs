use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_string_split_{}", name));
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
fn test_split_char_and_empty_segments() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Lossless empty segments: "a,,b" -> ["a", "", "b"]
    dec s1 = string_from_str("a,,b");
    dec parts1 = s1.split(',');
    if parts1.len() != (3 as u64) {
        return 1;
    }
    match parts1.get(0 as u64) {
        Option::Some(p) -> {
            if p.eq_str("a") == false {
                return 2;
            }
        },
        Option::None -> { return 3; },
    };
    match parts1.get(1 as u64) {
        Option::Some(p) -> {
            if p.eq_str("") == false {
                return 4;
            }
        },
        Option::None -> { return 5; },
    };
    match parts1.get(2 as u64) {
        Option::Some(p) -> {
            if p.eq_str("b") == false {
                return 6;
            }
        },
        Option::None -> { return 7; },
    };

    // 2. Leading and trailing delimiters
    dec s2 = string_from_str(",x,");
    dec parts2 = s2.split(',');
    if parts2.len() != (3 as u64) {
        return 8;
    }
    match parts2.get(0 as u64) {
        Option::Some(p) -> {
            if p.eq_str("") == false { return 9; }
        },
        Option::None -> { return 10; },
    };
    match parts2.get(1 as u64) {
        Option::Some(p) -> {
            if p.eq_str("x") == false { return 11; }
        },
        Option::None -> { return 12; },
    };
    match parts2.get(2 as u64) {
        Option::Some(p) -> {
            if p.eq_str("") == false { return 13; }
        },
        Option::None -> { return 14; },
    };

    // 3. Single delimiter "," -> ["", ""]
    dec s3 = string_from_str(",");
    dec parts3 = s3.split(',');
    if parts3.len() != (2 as u64) {
        return 15;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_split_char_and_empty_segments", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_split_str_and_split_string() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s = string_from_str("root::child::leaf");
    dec parts = s.split_str("::");
    if parts.len() != (3 as u64) {
        return 1;
    }
    match parts.get(0 as u64) {
        Option::Some(p) -> {
            if p.eq_str("root") == false { return 2; }
        },
        Option::None -> { return 3; },
    };
    match parts.get(1 as u64) {
        Option::Some(p) -> {
            if p.eq_str("child") == false { return 4; }
        },
        Option::None -> { return 5; },
    };
    match parts.get(2 as u64) {
        Option::Some(p) -> {
            if p.eq_str("leaf") == false { return 6; }
        },
        Option::None -> { return 7; },
    };

    dec delim = string_from_str("::");
    dec parts2 = s.split_string(&delim);
    if parts2.len() != (3 as u64) {
        return 8;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_split_str_and_split_string", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_lines_normalization() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. CRLF and LF mixed with trailing newline
    dec s = string_from_str("first\r\nsecond\nthird\r\n");
    dec l = s.lines();
    if l.len() != (3 as u64) {
        return 1;
    }
    match l.get(0 as u64) {
        Option::Some(line) -> {
            if line.eq_str("first") == false { return 2; }
        },
        Option::None -> { return 3; },
    };
    match l.get(1 as u64) {
        Option::Some(line) -> {
            if line.eq_str("second") == false { return 4; }
        },
        Option::None -> { return 5; },
    };
    match l.get(2 as u64) {
        Option::Some(line) -> {
            if line.eq_str("third") == false { return 6; }
        },
        Option::None -> { return 7; },
    };

    // 2. Interior blank line: "a\n\nb" -> ["a", "", "b"]
    dec s2 = string_from_str("a\n\nb");
    dec l2 = s2.lines();
    if l2.len() != (3 as u64) {
        return 8;
    }
    match l2.get(1 as u64) {
        Option::Some(line) -> {
            if line.eq_str("") == false { return 9; }
        },
        Option::None -> { return 10; },
    };

    // 3. Empty string -> 0 lines
    dec empty_s = string_new();
    dec empty_l = empty_s.lines();
    if empty_l.len() != (0 as u64) {
        return 11;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_lines_normalization", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_split_whitespace() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s = string_from_str("   hello \t world \r\n luna \t\t ");
    dec words = s.split_whitespace();
    if words.len() != (3 as u64) {
        return 1;
    }
    match words.get(0 as u64) {
        Option::Some(w) -> {
            if w.eq_str("hello") == false { return 2; }
        },
        Option::None -> { return 3; },
    };
    match words.get(1 as u64) {
        Option::Some(w) -> {
            if w.eq_str("world") == false { return 4; }
        },
        Option::None -> { return 5; },
    };
    match words.get(2 as u64) {
        Option::Some(w) -> {
            if w.eq_str("luna") == false { return 6; }
        },
        Option::None -> { return 7; },
    };

    // All whitespace string -> 0 words
    dec ws = string_from_str("  \t \r\n ");
    dec words_ws = ws.split_whitespace();
    if words_ws.len() != (0 as u64) {
        return 8;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_split_whitespace", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_trim_string() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s = string_from_str("  \t  hello luna!  \n ");
    dec trimmed = s.trim_string();
    if trimmed.eq_str("hello luna!") == false {
        return 1;
    }

    dec s_already_trimmed = string_from_str("clean");
    dec t2 = s_already_trimmed.trim_string();
    if t2.eq_str("clean") == false {
        return 2;
    }

    dec s_empty = string_from_str("    ");
    dec t3 = s_empty.trim_string();
    if t3.len() != (0 as u64) {
        return 3;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_trim_string", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}
