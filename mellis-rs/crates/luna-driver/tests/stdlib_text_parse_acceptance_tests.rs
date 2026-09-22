use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_text_parse_{}", name));
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
fn test_parse_signed_integers() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. parse_i32
    dec s1 = string_from_str("12345");
    match s1.parse_i32() {
        Option::Some(v) -> {
            if v != 12345 { return 1; }
        },
        Option::None -> { return 2; },
    };

    dec s_neg = string_from_str("-9876");
    match s_neg.parse_i32() {
        Option::Some(v) -> {
            if v != ((0 as i32) - (9876 as i32)) { return 3; }
        },
        Option::None -> { return 4; },
    };

    // 2. parse_i8
    dec s_i8 = string_from_str("-128");
    match s_i8.parse_i8() {
        Option::Some(v) -> {
            if v != ((0 as i8) - (128 as i8)) { return 5; }
        },
        Option::None -> { return 6; },
    };

    // 3. parse_i16
    dec s_i16 = string_from_str("32767");
    match s_i16.parse_i16() {
        Option::Some(v) -> {
            if v != (32767 as i16) { return 7; }
        },
        Option::None -> { return 8; },
    };

    // 4. parse_i64 MIN and MAX
    dec s_i64_min = string_from_str("-9223372036854775808");
    match s_i64_min.parse_i64() {
        Option::Some(v) -> {
            if v != ((0 as i64) - (9223372036854775807 as i64) - (1 as i64)) { return 9; }
        },
        Option::None -> { return 10; },
    };

    dec s_i64_max = string_from_str("9223372036854775807");
    match s_i64_max.parse_i64() {
        Option::Some(v) -> {
            if v != (9223372036854775807 as i64) { return 11; }
        },
        Option::None -> { return 12; },
    };

    // 5. parse_isize
    dec s_isize = string_from_str("-42");
    match s_isize.parse_isize() {
        Option::Some(v) -> {
            if v != ((0 as isize) - (42 as isize)) { return 13; }
        },
        Option::None -> { return 14; },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_parse_signed_integers", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_parse_unsigned_integers() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. parse_u8
    dec s_u8 = string_from_str("255");
    match s_u8.parse_u8() {
        Option::Some(v) -> {
            if v != (255 as u8) { return 1; }
        },
        Option::None -> { return 2; },
    };

    // 2. parse_u16
    dec s_u16 = string_from_str("65535");
    match s_u16.parse_u16() {
        Option::Some(v) -> {
            if v != (65535 as u16) { return 3; }
        },
        Option::None -> { return 4; },
    };

    // 3. parse_u32
    dec s_u32 = string_from_str("4294967295");
    match s_u32.parse_u32() {
        Option::Some(v) -> {
            if v != (4294967295 as u32) { return 5; }
        },
        Option::None -> { return 6; },
    };

    // 4. parse_u64 MAX
    dec s_u64 = string_from_str("18446744073709551615");
    match s_u64.parse_u64() {
        Option::Some(v) -> {
            // Check that v == u64::MAX without overflowing signed literal
            if (v + (1 as u64)) != (0 as u64) { return 7; }
        },
        Option::None -> { return 8; },
    };

    // 5. parse_usize
    dec s_usize = string_from_str("1024");
    match s_usize.parse_usize() {
        Option::Some(v) -> {
            if v != (1024 as usize) { return 9; }
        },
        Option::None -> { return 10; },
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_parse_unsigned_integers", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_parse_bool() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec s_t = string_from_str("true");
    match s_t.parse_bool() {
        Option::Some(b) -> {
            if b == false { return 1; }
        },
        Option::None -> { return 2; },
    };

    dec s_f = string_from_str("false");
    match s_f.parse_bool() {
        Option::Some(b) -> {
            if b { return 3; }
        },
        Option::None -> { return 4; },
    };

    dec s_inv = string_from_str("True");
    match s_inv.parse_bool() {
        Option::Some(_) -> { return 5; },
        Option::None -> {},
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_parse_bool", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_parse_overflow_rejections() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. u8 overflow: 256
    dec s1 = string_from_str("256");
    match s1.parse_u8() {
        Option::Some(_) -> { return 1; },
        Option::None -> {},
    };

    // 2. i8 overflow: 128
    dec s2 = string_from_str("128");
    match s2.parse_i8() {
        Option::Some(_) -> { return 2; },
        Option::None -> {},
    };

    // 3. i8 underflow: -129
    dec s3 = string_from_str("-129");
    match s3.parse_i8() {
        Option::Some(_) -> { return 3; },
        Option::None -> {},
    };

    // 4. u64 overflow: 18446744073709551616
    dec s4 = string_from_str("18446744073709551616");
    match s4.parse_u64() {
        Option::Some(_) -> { return 4; },
        Option::None -> {},
    };

    // 5. i64 overflow: 9223372036854775808
    dec s5 = string_from_str("9223372036854775808");
    match s5.parse_i64() {
        Option::Some(_) -> { return 5; },
        Option::None -> {},
    };

    // 6. i64 underflow: -9223372036854775809
    dec s6 = string_from_str("-9223372036854775809");
    match s6.parse_i64() {
        Option::Some(_) -> { return 6; },
        Option::None -> {},
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_parse_overflow_rejections", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_parse_syntax_rejections() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Empty string
    dec s_empty = string_new();
    match s_empty.parse_i32() {
        Option::Some(_) -> { return 1; },
        Option::None -> {},
    };

    // 2. Whitespace padded - should NOT trim implicitly
    dec s_ws = string_from_str(" 42 ");
    match s_ws.parse_i32() {
        Option::Some(_) -> { return 2; },
        Option::None -> {},
    };

    // 3. Letters in number
    dec s_let = string_from_str("42a");
    match s_let.parse_i32() {
        Option::Some(_) -> { return 3; },
        Option::None -> {},
    };

    // 4. Bare minus sign
    dec s_minus = string_from_str("-");
    match s_minus.parse_i32() {
        Option::Some(_) -> { return 4; },
        Option::None -> {},
    };

    // 5. Unsigned cannot have leading minus
    dec s_un_minus = string_from_str("-1");
    match s_un_minus.parse_u32() {
        Option::Some(_) -> { return 5; },
        Option::None -> {},
    };

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_parse_syntax_rejections", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}
