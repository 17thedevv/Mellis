use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::fs;
use colored::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to a .ms file or a directory containing .ms files
    #[arg(required = true)]
    path: String,

    /// Timeout in seconds for the generated executable
    #[arg(long, default_value_t = 5)]
    timeout: u64,
}

#[derive(Debug, PartialEq)]
enum TestResult {
    Pass,
    FailCompileDiff,
    FailRuntimeDiff,
}

fn main() {
    let args = Args::parse();
    let path = Path::new(&args.path);

    let cpp_compiler = r"D:\fdlang\bin\mellis.exe";
    let rust_compiler = r"D:\fdlang\mellis-rs\target\debug\mellis.exe";

    if !Path::new(cpp_compiler).exists() {
        eprintln!("C++ compiler not found at {}", cpp_compiler);
        std::process::exit(1);
    }
    if !Path::new(rust_compiler).exists() {
        eprintln!("Rust compiler not found at {}", rust_compiler);
        std::process::exit(1);
    }

    let cpp_tmp = Path::new(".diff-tmp/cpp");
    let rust_tmp = Path::new(".diff-tmp/rust");
    let failures_dir = Path::new(".diff-tmp/failures");
    let _ = fs::remove_dir_all(failures_dir);
    fs::create_dir_all(cpp_tmp).unwrap();
    fs::create_dir_all(rust_tmp).unwrap();
    fs::create_dir_all(failures_dir).unwrap();

    let mut files_to_test = Vec::new();
    if path.is_file() {
        files_to_test.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension().map_or(false, |ext| ext == "ms") {
                files_to_test.push(p);
            }
        }
    }

    let mut passed = 0;
    let mut failed = 0;
    
    let mut compile_fails = 0;
    let mut runtime_fails = 0;

    for test_file in &files_to_test {
        let result = run_differential_test(test_file, cpp_compiler, rust_compiler, cpp_tmp, rust_tmp, failures_dir, args.timeout);
        
        print!("Testing {}: ", test_file.display());
        match result {
            TestResult::Pass => {
                println!("{}", "PASS".green());
                passed += 1;
            }
            TestResult::FailCompileDiff => {
                println!("{}", "FAIL (Compile Phase)".red());
                failed += 1;
                compile_fails += 1;
            }
            TestResult::FailRuntimeDiff => {
                println!("{}", "FAIL (Runtime Phase)".red());
                failed += 1;
                runtime_fails += 1;
            }
        }
    }

    println!("\nSummary: {} passed, {} failed.", passed.to_string().green(), failed.to_string().red());
    if failed > 0 {
        println!("  Compile Fails: {}", compile_fails);
        println!("  Runtime Fails: {}", runtime_fails);
        std::process::exit(1);
    }
}

use std::sync::mpsc;
use std::thread;

fn run_with_timeout(mut child: std::process::Child, timeout_secs: u64) -> Result<std::process::ExitStatus, ()> {
    let timeout = Duration::from_secs(timeout_secs);
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        match child.wait() {
            Ok(status) => { let _ = tx.send(status); },
            Err(_) => {},
        }
    });

    match rx.recv_timeout(timeout) {
        Ok(status) => Ok(status),
        Err(_) => Err(())
    }
}

fn dump_failure(
    failures_dir: &Path, 
    base_name: &str, 
    test_file: &Path,
    phase: &str,
    cpp_exit: i32, rust_exit: i32,
    cpp_stdout: &str, cpp_stderr: &str,
    rust_stdout: &str, rust_stderr: &str
) {
    let fail_dir = failures_dir.join(base_name);
    let _ = fs::create_dir_all(&fail_dir);
    
    fs::write(fail_dir.join("cpp.exit"), format!("{}", cpp_exit)).unwrap();
    fs::write(fail_dir.join("rust.exit"), format!("{}", rust_exit)).unwrap();
    fs::write(fail_dir.join("cpp.stdout"), cpp_stdout).unwrap();
    fs::write(fail_dir.join("cpp.stderr"), cpp_stderr).unwrap();
    fs::write(fail_dir.join("rust.stdout"), rust_stdout).unwrap();
    fs::write(fail_dir.join("rust.stderr"), rust_stderr).unwrap();
    
    let summary = format!("Test: {}\n\nCompiler:\n  C++  exit={}\n  Rust exit={}\n\nClassification:\n  {}\n",
        test_file.display(), cpp_exit, rust_exit, phase);
    fs::write(fail_dir.join("summary.txt"), summary).unwrap();
}

fn run_differential_test(
    test_file: &Path,
    cpp_compiler: &str,
    rust_compiler: &str,
    cpp_tmp: &Path,
    rust_tmp: &Path,
    failures_dir: &Path,
    timeout_secs: u64,
) -> TestResult {
    let base_name = test_file.file_stem().unwrap().to_str().unwrap();

    let cpp_target_exe = cpp_tmp.join(format!("{}.exe", base_name));
    let rust_target_exe = rust_tmp.join(format!("{}.exe", base_name));

    let _ = fs::remove_file(&cpp_target_exe);
    let _ = fs::remove_file(&rust_target_exe);

    let cpp_compile = Command::new(cpp_compiler)
        .arg("--quiet")
        .arg(test_file)
        .current_dir(cpp_tmp)
        .env("PATH", format!("{};{}", std::env::var("PATH").unwrap_or_default(), "D:\\Programs\\LLVM-DEV\\bin"))
        .output()
        .expect("Failed to execute cpp compiler");

    let rust_compile = Command::new(rust_compiler)
        .arg("--quiet")
        .arg(test_file)
        .current_dir(rust_tmp)
        .env("LLVM_SYS_180_PREFIX", "D:\\Programs\\LLVM-DEV")
        .env("PATH", format!("{};{}", std::env::var("PATH").unwrap_or_default(), "D:\\Programs\\LLVM-DEV\\bin"))
        .env("MELLIS_RUNTIME_LIB", std::env::var("MELLIS_RUNTIME_LIB").unwrap_or_else(|_| "D:\\fdlang\\build\\runtime\\Release\\mellis-runtime.lib".to_string()))
        .output()
        .expect("Failed to execute rust compiler");

    let cpp_code = cpp_compile.status.code().unwrap_or(-1);
    let rust_code = rust_compile.status.code().unwrap_or(-1);

    // If both failed to compile, it's a pass for invalid tests, unless we do semantic normalization.
    // For now, assume parity if both failed or both succeeded.
    // Wait, exit codes don't have to match exactly. Both != 0 means both rejected.
    let cpp_success = cpp_compile.status.success();
    let rust_success = rust_compile.status.success();
    
    if cpp_success != rust_success {
        dump_failure(failures_dir, base_name, test_file, "CompilePhase", 
            cpp_code, rust_code, 
            &String::from_utf8_lossy(&cpp_compile.stdout), &String::from_utf8_lossy(&cpp_compile.stderr),
            &String::from_utf8_lossy(&rust_compile.stdout), &String::from_utf8_lossy(&rust_compile.stderr));
        return TestResult::FailCompileDiff;
    }

    if !cpp_success {
        // Both rejected. Assume parity for now.
        return TestResult::Pass;
    }

    // Stage 2: Runtime
    if !cpp_target_exe.exists() || !rust_target_exe.exists() {
        dump_failure(failures_dir, base_name, test_file, "CompilePhase / Executable Missing", 
            cpp_code, rust_code, 
            &String::from_utf8_lossy(&cpp_compile.stdout), 
            &String::from_utf8_lossy(&cpp_compile.stderr),
            &String::from_utf8_lossy(&rust_compile.stdout), 
            &String::from_utf8_lossy(&rust_compile.stderr));
        return TestResult::FailCompileDiff;
    }

    let run_exe_simple = |exe_path: &Path| -> Option<(i32, String, String)> {
        let out = Command::new(exe_path).output().ok()?;
        Some((
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        ))
    };

    let cpp_run = run_exe_simple(&cpp_target_exe);
    let rust_run = run_exe_simple(&rust_target_exe);

    match (cpp_run, rust_run) {
        (Some(cpp), Some(rust)) => {
            if cpp.0 != rust.0 || cpp.1 != rust.1 || cpp.2 != rust.2 {
                dump_failure(failures_dir, base_name, test_file, "RuntimePhase", 
                    cpp.0, rust.0, 
                    &cpp.1, &cpp.2,
                    &rust.1, &rust.2);
                return TestResult::FailRuntimeDiff;
            }
        }
        _ => {
            dump_failure(failures_dir, base_name, test_file, "RuntimePhase / Timeout", 
                -1, -1, 
                "Timeout", "Timeout",
                "Timeout", "Timeout");
            return TestResult::FailRuntimeDiff;
        }
    }

    TestResult::Pass
}
