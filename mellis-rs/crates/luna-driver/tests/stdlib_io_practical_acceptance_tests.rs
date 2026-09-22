use std::{fs, io::Write, path::PathBuf, process::{Command, Stdio}};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_io_practical_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_and_run(test_name: &str, source: &str, stdin_data: Option<&str>) -> (i32, String, String) {
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

    let mut cmd = Command::new(&exe_path);
    if stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to spawn produced binary");

    if let Some(input) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).unwrap();
        }
    }

    let output = child.wait_with_output().expect("Failed to wait on child process");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// STDLIB-GAP-03: Test 1
/// Whole-file binary roundtrip: write bytes to file, read bytes from file, verify identical bytes.
#[test]
fn test_io_write_and_read_bytes_roundtrip() {
    let temp = create_temp_dir("bytes_roundtrip");
    let test_file = temp.join("test_data.bin");
    let test_file_str = test_file.to_str().unwrap().replace('\\', "/");

    let src = format!(r#"
import <io>;
import <result>;
import <string>;
import <vec>;

fn main() -> i32 {{
    dec path = string_from_str("{}");

    // Construct 4 test bytes: [65, 66, 67, 68] ('A', 'B', 'C', 'D')
    dec rw write_vec = vec_new<u8>();
    write_vec.push(65 as u8);
    write_vec.push(66 as u8);
    write_vec.push(67 as u8);
    write_vec.push(68 as u8);

    dec write_res = io::write(&path, write_vec.as_slice());
    match write_res {{
        Result::Ok(_) -> {{}},
        Result::Err(_) -> {{ return 1; }},
    }}

    dec read_res = io::read(&path);
    match read_res {{
        Result::Ok(read_vec) -> {{
            if read_vec.len() != (4 as u64) {{
                return 2;
            }}
            dec b0 = match read_vec.get(0 as u64) {{ Option::Some(v) -> *v, Option::None -> 0 as u8 }};
            dec b1 = match read_vec.get(1 as u64) {{ Option::Some(v) -> *v, Option::None -> 0 as u8 }};
            dec b2 = match read_vec.get(2 as u64) {{ Option::Some(v) -> *v, Option::None -> 0 as u8 }};
            dec b3 = match read_vec.get(3 as u64) {{ Option::Some(v) -> *v, Option::None -> 0 as u8 }};

            if b0 != (65 as u8) || b1 != (66 as u8) || b2 != (67 as u8) || b3 != (68 as u8) {{
                return 3;
            }}
            return 0;
        }},
        Result::Err(_) -> {{ return 4; }},
    }}
}}
"#, test_file_str);

    let (code, _, stderr) = compile_and_run("test_io_bytes_roundtrip", &src, None);
    assert_eq!(code, 0, "Test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-03: Test 2
/// Whole-file text roundtrip: write UTF-8 text with multibyte characters, read text, verify equality.
#[test]
fn test_io_write_and_read_text_roundtrip() {
    let temp = create_temp_dir("text_roundtrip");
    let test_file = temp.join("test_text.txt");
    let test_file_str = test_file.to_str().unwrap().replace('\\', "/");

    let src = format!(r#"
import <io>;
import <result>;
import <string>;

fn main() -> i32 {{
    dec path = string_from_str("{}");
    dec text_to_write = string_from_str("Luna: Xin chào thế giới! 🚀");

    dec write_res = io::write_text(&path, &text_to_write);
    match write_res {{
        Result::Ok(_) -> {{}},
        Result::Err(_) -> {{ return 1; }},
    }}

    dec read_res = io::read_text(&path);
    match read_res {{
        Result::Ok(read_str) -> {{
            if read_str.eq_string(&text_to_write) == false {{
                return 2;
            }}
            return 0;
        }},
        Result::Err(_) -> {{ return 3; }},
    }}
}}
"#, test_file_str);

    let (code, _, stderr) = compile_and_run("test_io_text_roundtrip", &src, None);
    assert_eq!(code, 0, "Test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-03: Test 3
/// File not found: reading a non-existent file path must return IoError::NotFound.
#[test]
fn test_io_read_not_found() {
    let temp = create_temp_dir("not_found");
    let non_existent = temp.join("definitely_not_here.txt");
    let path_str = non_existent.to_str().unwrap().replace('\\', "/");

    let src = format!(r#"
import <io>;
import <result>;
import <string>;

fn main() -> i32 {{
    dec path = string_from_str("{}");
    dec res = io::read_text(&path);
    match res {{
        Result::Ok(_) -> {{
            return 1;
        }},
        Result::Err(err) -> {{
            match err {{
                io::IoError::NotFound -> {{
                    return 0;
                }},
                io::IoError::PermissionDenied -> {{ return 2; }},
                io::IoError::AlreadyExists -> {{ return 3; }},
                io::IoError::InvalidInput -> {{ return 4; }},
                io::IoError::InvalidData -> {{ return 5; }},
                io::IoError::OutOfMemory -> {{ return 6; }},
                io::IoError::Other(_) -> {{ return 7; }},
            }}
        }},
    }}
}}
"#, path_str);

    let (code, _, stderr) = compile_and_run("test_io_not_found", &src, None);
    assert_eq!(code, 0, "Test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-02: Test 4
/// Stdin line input: reads multiple lines, verifies CRLF/newline stripping and EOF handling.
#[test]
fn test_io_read_line_piped() {
    let src = r#"
import <io>;
import <result>;
import <string>;

fn main() -> i32 {
    // Line 1: "first line"
    dec l1_res = io::read_line();
    match l1_res {
        Result::Ok(opt1) -> {
            match opt1 {
                Option::Some(s1) -> {
                    dec expected1 = string_from_str("first line");
                    if s1.eq_string(&expected1) == false {
                        return 1;
                    }
                },
                Option::None -> { return 2; },
            }
        },
        Result::Err(_) -> { return 3; },
    }

    // Line 2: "second line"
    dec l2_res = io::read_line();
    match l2_res {
        Result::Ok(opt2) -> {
            match opt2 {
                Option::Some(s2) -> {
                    dec expected2 = string_from_str("second line");
                    if s2.eq_string(&expected2) == false {
                        return 4;
                    }
                },
                Option::None -> { return 5; },
            }
        },
        Result::Err(_) -> { return 6; },
    }

    // Line 3: EOF
    dec l3_res = io::read_line();
    match l3_res {
        Result::Ok(opt3) -> {
            match opt3 {
                Option::Some(_) -> { return 7; },
                Option::None -> {
                    // EOF reached as expected
                    return 0;
                },
            }
        },
        Result::Err(_) -> { return 8; },
    }
}
"#;

    let input = "first line\r\nsecond line\n";
    let (code, _, stderr) = compile_and_run("test_io_read_line", src, Some(input));
    assert_eq!(code, 0, "Test failed with code {}. Stderr: {}", code, stderr);
}

/// High-level string printing: print_string, println_string, eprintln_string.
#[test]
fn test_io_string_printing() {
    let src = r#"
import <io>;
import <string>;

fn main() -> i32 {
    dec hello = string_from_str("Hello, ");
    dec world = string_from_str("World!");
    dec err_msg = string_from_str("Error message");

    io::print_string(&hello);
    io::println_string(&world);
    io::eprintln_string(&err_msg);

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_io_printing", src, None);
    assert_eq!(code, 0, "Test failed with code {}. Stderr: {}", code, stderr);
    assert_eq!(stdout.replace("\r\n", "\n"), "Hello, World!\n");
    assert_eq!(stderr.replace("\r\n", "\n"), "Error message\n");
}
