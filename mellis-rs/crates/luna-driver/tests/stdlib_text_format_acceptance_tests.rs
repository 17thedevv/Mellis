use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_text_format_{}", name));
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
fn test_format_bool_and_char() {
    let src = r#"
import <string>;

fn main() -> i32 {
    dec st = string_from_bool(true);
    if st.eq_str("true") == false { return 1; }

    dec sf = string_from_bool(false);
    if sf.eq_str("false") == false { return 2; }

    dec sc = string_from_char('Z');
    if sc.eq_str("Z") == false { return 3; }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_format_bool_and_char", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_format_unsigned_integers() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Zero
    dec s0 = string_from_u64(0 as u64);
    if s0.eq_str("0") == false { return 1; }

    // 2. u8
    dec su8 = string_from_u8(255 as u8);
    if su8.eq_str("255") == false { return 2; }

    // 3. u16
    dec su16 = string_from_u16(65535 as u16);
    if su16.eq_str("65535") == false { return 3; }

    // 4. u32
    dec su32 = string_from_u32(4294967295 as u32);
    if su32.eq_str("4294967295") == false { return 4; }

    // 5. u64 MAX
    dec su64 = string_from_u64(18446744073709551615 as u64);
    if su64.eq_str("18446744073709551615") == false { return 5; }

    // 6. usize
    dec susize = string_from_usize(123456 as usize);
    if susize.eq_str("123456") == false { return 6; }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_format_unsigned_integers", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

#[test]
fn test_format_signed_integers() {
    let src = r#"
import <string>;

fn main() -> i32 {
    // 1. Zero
    dec s0 = string_from_i64(0 as i64);
    if s0.eq_str("0") == false { return 1; }

    // 2. Positive
    dec sp = string_from_i32(12345 as i32);
    if sp.eq_str("12345") == false { return 2; }

    // 3. Negative i8
    dec si8 = string_from_i8((0 as i8) - (128 as i8));
    if si8.eq_str("-128") == false { return 3; }

    // 4. Negative i16
    dec si16 = string_from_i16((0 as i16) - (32768 as i16));
    if si16.eq_str("-32768") == false { return 4; }

    // 5. Negative i32 MIN
    dec si32 = string_from_i32(((0 as i32) - (2147483647 as i32)) - (1 as i32));
    if si32.eq_str("-2147483648") == false { return 5; }

    // 6. Negative i64 MIN
    dec si64 = string_from_i64(((0 as i64) - (9223372036854775807 as i64)) - (1 as i64));
    if si64.eq_str("-9223372036854775808") == false { return 6; }

    // 7. Positive i64 MAX
    dec si64_max = string_from_i64(9223372036854775807 as i64);
    if si64_max.eq_str("9223372036854775807") == false { return 7; }

    // 8. isize
    dec sisize = string_from_isize((0 as isize) - (42 as isize));
    if sisize.eq_str("-42") == false { return 8; }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_format_signed_integers", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}
