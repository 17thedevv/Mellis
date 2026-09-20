use luna_driver::CompilerOptions;
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_phase_c_text_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
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

    let compile_res = luna_driver::compile_and_render(main_path.to_str().unwrap(), source.to_string(), &options);
    if let Err(rendered) = &compile_res {
        eprintln!("RENDERED DIAGNOSTIC:\n{}", rendered);
    }
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

/// 1. UTF-8 construction, validation, and invariants
#[test]
fn test_string_utf8_invariants_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <string>;

fn main() -> i32 {
    // Valid from str
    dec rw s = string_from_str("Luna UTF-8");
    if s.len() != (10 as u64) { return 1; }

    // Push char
    s.push_char('!');
    if s.len() != (11 as u64) { return 2; }

    // Push str
    s.push_str(" Verified");
    if s.len() != (20 as u64) { return 3; }

    // Fallible from_bytes on valid UTF-8
    dec valid_bytes: [u8; 4] = [240 as u8, 159 as u8, 146 as u8, 150 as u8]; // 💖
    dec opt_valid = string_from_bytes(&valid_bytes);
    if opt_valid.is_some() == false { return 4; }

    // Fallible from_bytes on invalid UTF-8
    dec invalid_bytes: [u8; 2] = [255 as u8, 255 as u8];
    dec opt_invalid = string_from_bytes(&invalid_bytes);
    if opt_invalid.is_some() == true { return 5; }

    // Truncate at ASCII char boundary
    s.truncate(4 as u64); // "Luna"
    if s.len() != (4 as u64) { return 6; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_string_utf8_invariants_e2e", src);
    assert_eq!(code, 0, "test_string_utf8_invariants_e2e failed with code {}", code);
}

/// 2. Integer formatting (ToString) and parsing
#[test]
fn test_string_integer_formatting_and_parsing_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <string>;

fn main() -> i32 {
    // 1. string_from_i32 formatting
    dec s_zero = string_from_i32(0);
    if s_zero.len() != (1 as u64) { return 1; }
    if s_zero.parse_i32().unwrap() != 0 { return 2; }

    dec s_pos = string_from_i32(12345);
    if s_pos.len() != (5 as u64) { return 3; }
    if s_pos.parse_i32().unwrap() != 12345 { return 4; }

    dec s_neg = string_from_i32(0 - 9876);
    if s_neg.len() != (5 as u64) { return 5; }
    if s_neg.parse_i32().unwrap() != (0 - 9876) { return 6; }

    // 2. parse_i32 on string_from_str
    dec s_answer = string_from_str("42");
    if s_answer.parse_i32().unwrap() != 42 { return 7; }

    dec s_neg_answer = string_from_str("-100");
    if s_neg_answer.parse_i32().unwrap() != (0 - 100) { return 8; }

    // 3. invalid parse returns None
    dec s_invalid = string_from_str("abc");
    if s_invalid.parse_i32().is_some() == true { return 9; }

    dec s_empty = string_new();
    if s_empty.parse_i32().is_some() == true { return 10; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_string_integer_formatting_and_parsing_e2e", src);
    assert_eq!(code, 0, "test_string_integer_formatting_and_parsing_e2e failed with code {}", code);
}

/// 3. String whitespace trimming
#[test]
fn test_string_trim_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <string>;

fn main() -> i32 {
    dec s = string_from_str("   hello world   ");
    dec trimmed = s.trim();

    // "hello world" length is 11
    if (trimmed.len as u64) != (11 as u64) { return 1; }
    if trimmed[0] != (104 as u8) { return 2; } // 'h'
    if trimmed[10 as u64] != (100 as u8) { return 3; } // 'd'

    // Trimming all whitespace
    dec s_spaces = string_from_str("    ");
    dec trimmed_spaces = s_spaces.trim();
    if (trimmed_spaces.len as u64) != (0 as u64) { return 4; }

    // Trimming already clean
    dec s_clean = string_from_str("clean");
    dec trimmed_clean = s_clean.trim();
    if (trimmed_clean.len as u64) != (5 as u64) { return 5; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_string_trim_e2e", src);
    assert_eq!(code, 0, "test_string_trim_e2e failed with code {}", code);
}

/// 4. String clone and equality independence
#[test]
fn test_string_clone_and_eq_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;
import <string>;

fn main() -> i32 {
    dec s1 = string_from_str("Mellis");
    dec rw s2 = s1.clone();

    if s1.eq(&s2) == false { return 1; }

    s2.push_str(" Compiler");
    if s1.eq(&s2) == true { return 2; }
    if s1.len() != (6 as u64) { return 3; }
    if s2.len() != (15 as u64) { return 4; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_string_clone_and_eq_e2e", src);
    assert_eq!(code, 0, "test_string_clone_and_eq_e2e failed with code {}", code);
}
