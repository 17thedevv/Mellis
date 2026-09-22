use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("luna_c_gap_11_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_and_run_with_args(test_name: &str, source: &str, args: &[&str]) -> (i32, String, String) {
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
        .args(args)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// C11-1: Real arguments delivered
/// fn main(args: [str]) receives actual non-empty CLI arguments with correct count and values.
#[test]
fn test_c11_1_real_arguments_delivered() {
    let src = r#"
import <string>;

fn main(args: [str]) -> i32 {
    if args.length != (2 as usize) {
        return 1;
    }
    dec a0 = string_from_str(args[0]);
    dec a1 = string_from_str(args[1]);
    if a0.eq_str("hello") == false {
        return 2;
    }
    if a1.eq_str("world") == false {
        return 3;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args("test_c11_1", src, &["hello", "world"]);
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);
}

/// C11-2: Empty argument vector
/// Launch with zero user arguments produces a valid empty slice (args.length == 0).
#[test]
fn test_c11_2_empty_argument_vector() {
    let src = r#"
fn main(args: [str]) -> i32 {
    if args.length != (0 as usize) {
        return 1;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args("test_c11_2", src, &[]);
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);
}

/// C11-3: Executable name excluded
/// args contains strictly user arguments; native argv[0] is not included.
#[test]
fn test_c11_3_executable_name_excluded() {
    let src = r#"
import <string>;

fn main(args: [str]) -> i32 {
    // Only 1 user argument was passed; args.length must be exactly 1
    if args.length != (1 as usize) {
        return 1;
    }
    dec a0 = string_from_str(args[0]);
    // The argument must be the supplied user argument, never the executable path
    if a0.eq_str("only_user_arg") == false {
        return 2;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args("test_c11_3", src, &["only_user_arg"]);
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);
}

/// C11-4: Ordering
/// Arguments alpha, beta, gamma, delta arrive in exact sequence.
#[test]
fn test_c11_4_argument_ordering_preserved() {
    let src = r#"
import <string>;

fn main(args: [str]) -> i32 {
    if args.length != (4 as usize) {
        return 1;
    }
    if string_from_str(args[0]).eq_str("alpha") == false {
        return 2;
    }
    if string_from_str(args[1]).eq_str("beta") == false {
        return 3;
    }
    if string_from_str(args[2]).eq_str("gamma") == false {
        return 4;
    }
    if string_from_str(args[3]).eq_str("delta") == false {
        return 5;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args("test_c11_4", src, &["alpha", "beta", "gamma", "delta"]);
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);
}

/// C11-5: Spaces and formatting
/// Arguments with spaces, tabs, and symbols survive uncorrupted.
#[test]
fn test_c11_5_spaces_and_formatting_preserved() {
    let src = r#"
import <string>;

fn main(args: [str]) -> i32 {
    if args.length != (3 as usize) {
        return 1;
    }
    if string_from_str(args[0]).eq_str("hello world") == false {
        return 2;
    }
    if string_from_str(args[1]).eq_str("tab\tseparated") == false {
        return 3;
    }
    if string_from_str(args[2]).eq_str("--flag=value with spaces") == false {
        return 4;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args(
        "test_c11_5",
        src,
        &["hello world", "tab\tseparated", "--flag=value with spaces"],
    );
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);
}

/// C11-6: Unicode normalized UTF-8 delivery
/// Non-ASCII characters (Vietnamese, Japanese, Emoji) arrive correctly in UTF-8.
#[test]
fn test_c11_6_unicode_utf8_preserved() {
    let src = r#"
import <io>;
import <string>;

fn main(args: [str]) -> i32 {
    dec rw i: usize = 0 as usize;
    while i < args.length {
        dec s = string_from_str(args[i]);
        io::println(s.as_bytes());
        i = i + (1 as usize);
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run_with_args(
        "test_c11_6",
        src,
        &["Tiếng Việt", "こんにちは", "🦀"],
    );
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);

    let normalized_stdout = stdout.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized_stdout.trim_end().split('\n').collect();
    assert_eq!(lines, vec!["Tiếng Việt", "こんにちは", "🦀"]);
}

/// C11-7: Exact launcher-vector preservation
/// The argument vector observed by main(args) exactly matches the launcher vector in count, ordering, and content.
#[test]
fn test_c11_7_exact_launcher_vector_preservation() {
    let src = r#"
import <io>;
import <string>;

fn main(args: [str]) -> i32 {
    dec rw i: usize = 0 as usize;
    while i < args.length {
        dec s = string_from_str(args[i]);
        io::println(s.as_bytes());
        i = i + (1 as usize);
    }
    return 0;
}
"#;
    let expected = &["first", "second with spaces", "Tiếng Việt 🇻🇳", "end"];
    let (code, stdout, stderr) = compile_and_run_with_args("test_c11_7", src, expected);
    assert_eq!(code, 0, "Exit code {}. Stderr: {}. Stdout: {}", code, stderr, stdout);

    let normalized_stdout = stdout.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized_stdout.trim_end().split('\n').collect();
    assert_eq!(lines, *expected);
}

/// C11-8: Existing main forms unaffected
/// fn main() -> void and fn main() -> i32 retain their established semantics unchanged.
#[test]
fn test_c11_8_existing_main_forms_unaffected() {
    // 1. fn main() -> void with args passed on CLI
    let src_void = r#"
fn main() {}
"#;
    let (code_void, _, stderr_void) = compile_and_run_with_args("test_c11_8_void", src_void, &["ignored", "args"]);
    assert_eq!(code_void, 0, "fn main() -> void must return 0; stderr: {}", stderr_void);

    // 2. fn main() -> i32 with args passed on CLI
    let src_i32 = r#"
fn main() -> i32 {
    return 42;
}
"#;
    let (code_i32, _, stderr_i32) = compile_and_run_with_args("test_c11_8_i32", src_i32, &["ignored", "args"]);
    assert_eq!(code_i32, 42, "fn main() -> i32 must propagate return value; stderr: {}", stderr_i32);
}
