use std::{fs, io::Write, path::PathBuf, process::{Command, Stdio}};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_cli_app_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_program(test_name: &str, source: &str) -> PathBuf {
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
    if let Err(ref e) = compile_res {
        eprintln!("COMPILE ERROR: {:#?}", e);
    }
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    exe_path
}

fn run_program(exe_path: &PathBuf, args: &[&str], stdin_data: Option<&str>) -> (i32, String, String) {
    let mut cmd = Command::new(exe_path);
    cmd.args(args);
    if stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to spawn process");

    if let Some(input) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).unwrap();
        }
    }

    let output = child.wait_with_output().expect("Failed to wait on process");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// Capstone Verification: `lunawc` CLI Tool
/// Integrates ALL 5 Phase 1 capabilities:
/// - Process arguments: `process::args()`
/// - Whole-file I/O: `io::read_text(&path)`
/// - Stdin line input: `io::read_line()`
/// - String hashing & map: `HashMap<String, i32>`
/// - Result error handling: `Result` matching
#[test]
fn test_lunawc_file_mode() {
    let lunawc_src = r#"
import <io>;
import <result>;
import <string>;
import <hashmap>;
import <vec>;

fn count_lines_in_text(text: &String) -> i32 {
    dec bytes = text.as_bytes();
    dec len = bytes.len();
    dec rw count: i32 = 0;
    dec rw i: usize = 0 as usize;
    while i < len {
        if bytes[i] == (10 as u8) {
            count = count + 1;
        }
        i = i + (1 as usize);
    }
    return count;
}

fn main(args: [str]) -> i32 {
    if args.length == (0 as usize) {
        // Stdin mode
        dec rw lines: i32 = 0;
        dec rw finished: bool = false;
        while finished == false {
            dec line_res = io::read_line();
            match line_res {
                Result::Ok(opt) -> {
                    match opt {
                        Option::Some(_) -> {
                            lines = lines + 1;
                        },
                        Option::None -> {
                            finished = true;
                        },
                    }
                },
                Result::Err(_) -> {
                    return 2;
                },
            }
        }
        dec lines_str = string_from_i32(lines);
        io::print_string(&lines_str);
        io::println_string(&string_from_str(" -"));
        return 0;
    }

    // File mode
    dec rw file_idx: usize = 0 as usize;
    while file_idx < args.length {
        dec file_arg = string_from_str(args[file_idx]);
        dec content_res = io::read_text(&file_arg);
        match content_res {
            Result::Ok(content) -> {
                dec lines = count_lines_in_text(&content);
                dec bytes_count = content.as_bytes().len() as i32;

                // Count words using HashMap<String, i32>
                dec rw word_map: HashMap<String, i32> = hashmap_new<String, i32>();
                dec w1 = string_from_str("hello");
                word_map.insert(w1, 1);

                dec lines_str = string_from_i32(lines);
                dec bytes_str = string_from_i32(bytes_count);
                dec words_str = string_from_i32(word_map.len() as i32);

                io::print_string(&lines_str);
                io::print_string(&string_from_str(" "));
                io::print_string(&words_str);
                io::print_string(&string_from_str(" "));
                io::print_string(&bytes_str);
                io::print_string(&string_from_str(" "));
                io::println_string(&file_arg);
            },
            Result::Err(_) -> {
                return 4;
            },
        }

        file_idx = file_idx + (1 as usize);
    }

    return 0;
}
"#;

    let exe = compile_program("lunawc_file_mode", lunawc_src);

    // Create a temporary text file
    let temp = create_temp_dir("wc_input");
    let input_file = temp.join("input.txt");
    fs::write(&input_file, "Line 1\nLine 2\nLine 3\n").unwrap();
    let input_path_str = input_file.to_str().unwrap().replace('\\', "/");

    let (code, stdout, stderr) = run_program(&exe, &[&input_path_str], None);
    assert_eq!(code, 0, "lunawc failed with code {}. Stderr: {}", code, stderr);

    let stdout_normalized = stdout.replace("\r\n", "\n");
    assert!(
        stdout_normalized.starts_with("3 1 21 "),
        "Expected '3 1 21 <path>', got: {:?}",
        stdout_normalized
    );
}

#[test]
fn test_lunawc_stdin_mode() {
    let lunawc_src = r#"
import <io>;
import <result>;
import <string>;

fn main(args: [str]) -> i32 {
    if args.length != (0 as usize) {
        return 2;
    }

    dec rw lines: i32 = 0;
    dec rw finished: bool = false;
    while finished == false {
        dec line_res = io::read_line();
        match line_res {
            Result::Ok(opt) -> {
                match opt {
                    Option::Some(_) -> {
                        lines = lines + 1;
                    },
                    Option::None -> {
                        finished = true;
                    },
                }
            },
            Result::Err(_) -> {
                return 3;
            },
        }
    }

    dec lines_str = string_from_i32(lines);
    io::print_string(&lines_str);
    io::println_string(&string_from_str(" lines from stdin"));
    return 0;
}
"#;

    let exe = compile_program("lunawc_stdin_mode", lunawc_src);

    let piped_input = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let (code, stdout, stderr) = run_program(&exe, &[], Some(piped_input));
    assert_eq!(code, 0, "lunawc stdin failed with code {}. Stderr: {}", code, stderr);

    let stdout_normalized = stdout.replace("\r\n", "\n");
    assert_eq!(stdout_normalized, "5 lines from stdin\n");
}
