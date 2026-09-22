use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_text_maturity_{}", name));
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
fn test_adversarial_search_and_utf8() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Multibyte Vietnamese search and offset
    dec s_vn = string_from_str("Ngôn ngữ lập trình Luna!");
    if s_vn.contains("lập trình") == false { return 1; }
    if s_vn.contains("Tiếng") { return 2; }
    if s_vn.starts_with("Ngôn") == false { return 3; }
    if s_vn.ends_with("Luna!") == false { return 4; }

    // 2. Japanese characters
    dec s_jp = string_from_str("日本語プログラミング言語");
    if s_jp.contains("プログラミング") == false { return 5; }
    if s_jp.starts_with("日本語") == false { return 6; }
    if s_jp.ends_with("言語") == false { return 7; }

    // 3. Emojis (4-byte UTF-8 sequences)
    dec s_emoji = string_from_str("Luna is fast! 🚀✨🎉");
    if s_emoji.contains("🚀") == false { return 8; }
    if s_emoji.contains("✨🎉") == false { return 9; }
    if s_emoji.ends_with("🎉") == false { return 10; }

    // 4. Exact equality of entire string
    if s_emoji.contains("Luna is fast! 🚀✨🎉") == false { return 11; }
    match s_emoji.find("Luna is fast! 🚀✨🎉") {
        Option::Some(pos) -> {
            if pos != (0 as u64) { return 12; }
        },
        Option::None -> { return 13; },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_adversarial_search_and_utf8", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_adversarial_split_pathological() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Only delimiters: ",,,," (4 commas -> 5 empty strings)
    dec s_commas = string_from_str(",,,,");
    dec parts_c = s_commas.split(',');
    if parts_c.len() != (5 as u64) { return 1; }
    dec rw i: u64 = 0 as u64;
    while i < (5 as u64) {
        match parts_c.get(i) {
            Option::Some(p) -> {
                if p.len() != (0 as u64) { return 2; }
            },
            Option::None -> { return 3; },
        };
        i = i + (1 as u64);
    }

    // 2. No delimiter present: "hello_world" split on ':' -> 1 element
    dec s_no_delim = string_from_str("hello_world");
    dec parts_nd = s_no_delim.split(':');
    if parts_nd.len() != (1 as u64) { return 4; }
    match parts_nd.get(0 as u64) {
        Option::Some(p) -> {
            if p.eq_str("hello_world") == false { return 5; }
        },
        Option::None -> { return 6; },
    };

    // 3. Splitting with UTF-8 content preserved around ASCII delimiter
    dec s_utf8_split = string_from_str("Hà Nội|Đà Nẵng|Sài Gòn");
    dec cities = s_utf8_split.split('|');
    if cities.len() != (3 as u64) { return 7; }
    match cities.get(0 as u64) {
        Option::Some(c) -> {
            if c.eq_str("Hà Nội") == false { return 8; }
        },
        Option::None -> { return 9; },
    };
    match cities.get(1 as u64) {
        Option::Some(c) -> {
            if c.eq_str("Đà Nẵng") == false { return 10; }
        },
        Option::None -> { return 11; },
    };
    match cities.get(2 as u64) {
        Option::Some(c) -> {
            if c.eq_str("Sài Gòn") == false { return 12; }
        },
        Option::None -> { return 13; },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_adversarial_split_pathological", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_adversarial_lines_and_whitespace() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Multiple consecutive newlines: "\n\n\n" -> ["", "", ""]
    dec s_newlines = string_from_str("\n\n\n");
    dec lines_nl = s_newlines.lines();
    if lines_nl.len() != (3 as u64) { return 1; }

    // 2. CRLF mixed with bare LF
    dec s_mixed = string_from_str("alpha\r\n\nbeta\r\n\r\ngamma\n");
    dec lines_m = s_mixed.lines();
    if lines_m.len() != (5 as u64) { return 2; }
    match lines_m.get(0 as u64) {
        Option::Some(l) -> { if l.eq_str("alpha") == false { return 3; } },
        Option::None -> { return 4; },
    };
    match lines_m.get(1 as u64) {
        Option::Some(l) -> { if l.eq_str("") == false { return 5; } },
        Option::None -> { return 6; },
    };
    match lines_m.get(2 as u64) {
        Option::Some(l) -> { if l.eq_str("beta") == false { return 7; } },
        Option::None -> { return 8; },
    };
    match lines_m.get(3 as u64) {
        Option::Some(l) -> { if l.eq_str("") == false { return 9; } },
        Option::None -> { return 10; },
    };
    match lines_m.get(4 as u64) {
        Option::Some(l) -> { if l.eq_str("gamma") == false { return 11; } },
        Option::None -> { return 12; },
    };

    // 3. Complex whitespace with form feed (0x0C) and vertical tab (0x0B)
    dec s_ws_complex = string_from_str("one\t\ttwo   three\r\nfour");
    dec words = s_ws_complex.split_whitespace();
    if words.len() != (4 as u64) { return 13; }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_adversarial_lines_and_whitespace", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_adversarial_parse_and_format_boundaries() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Leading zeros in parsing
    dec s_lz = string_from_str("000042");
    match s_lz.parse_i32() {
        Option::Some(v) -> {
            if v != 42 { return 1; }
        },
        Option::None -> { return 2; },
    };

    // 2. Unsigned zero with leading zeros
    dec s_lz_u = string_from_str("00000");
    match s_lz_u.parse_u64() {
        Option::Some(v) -> {
            if v != (0 as u64) { return 3; }
        },
        Option::None -> { return 4; },
    };

    // 3. Negative zero "-0" parses as 0
    dec s_neg_zero = string_from_str("-0");
    match s_neg_zero.parse_i32() {
        Option::Some(v) -> {
            if v != 0 { return 5; }
        },
        Option::None -> { return 6; },
    };

    // 4. Format-parse roundtrip on boundary values
    dec max_u64 = 18446744073709551615 as u64;
    dec formatted_u64 = string_from_u64(max_u64);
    match formatted_u64.parse_u64() {
        Option::Some(parsed_back) -> {
            if parsed_back != max_u64 { return 7; }
        },
        Option::None -> { return 8; },
    };

    dec min_i64 = ((0 as i64) - (9223372036854775807 as i64)) - (1 as i64);
    dec formatted_i64 = string_from_i64(min_i64);
    match formatted_i64.parse_i64() {
        Option::Some(parsed_back) -> {
            if parsed_back != min_i64 { return 9; }
        },
        Option::None -> { return 10; },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_adversarial_parse_and_format_boundaries", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}
