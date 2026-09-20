use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default)]
struct TestDirectives {
    expect_accept: bool,
    expect_reject_patterns: Vec<String>,
    expect_exit: Option<i32>,
    expect_stdout: Option<String>,
}

fn parse_directives(source: &str) -> TestDirectives {
    let mut directives = TestDirectives::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("// [STRESS: COMPILE_ACCEPT]") {
            directives.expect_accept = true;
        } else if let Some(rest) = trimmed.strip_prefix("// [STRESS: COMPILE_REJECT:") {
            if let Some(pat) = rest.strip_suffix(']') {
                directives.expect_reject_patterns.push(pat.trim().to_string());
            }
        } else if let Some(idx) = trimmed.find("//~ ERROR") {
            let rest = &trimmed[idx + "//~ ERROR".len()..];
            directives.expect_reject_patterns.push(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("// [STRESS: EXEC_EXIT:") {
            if let Some(code_str) = rest.strip_suffix(']') {
                if let Ok(code) = code_str.trim().parse::<i32>() {
                    directives.expect_exit = Some(code);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("// [STRESS: EXEC_STDOUT:") {
            if let Some(out_str) = rest.strip_suffix(']') {
                let unquoted = out_str.trim().trim_matches('"').replace("\\n", "\n");
                directives.expect_stdout = Some(unquoted);
            }
        }
    }

    directives
}

fn run_stress_case(file_path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

    let directives = parse_directives(&source);

    let sysroot = Sysroot::discover_for_test()
        .map_err(|e| format!("Failed to discover test sysroot: {:?}", e))?;

    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let check_res = check(file_path.to_str().unwrap(), source.clone(), &options);

    // Negative Test Assertion
    if !directives.expect_reject_patterns.is_empty() {
        match check_res {
            Ok(_) => {
                return Err(format!(
                    "Expected REJECT with pattern(s) {:?}, but compilation SUCCEEDED.",
                    directives.expect_reject_patterns
                ));
            }
            Err(diags) => {
                for pat in &directives.expect_reject_patterns {
                    let found = diags.iter().any(|d| {
                        let code_str = d.code.map(|c| c.to_string()).unwrap_or_default();
                        d.message.contains(pat) || code_str.contains(pat)
                    });
                    if !found {
                        let actual_msgs: Vec<String> = diags
                            .iter()
                            .map(|d| {
                                format!(
                                    "[{}] {}",
                                    d.code.map(|c| c.to_string()).unwrap_or_else(|| "NO_CODE".to_string()),
                                    d.message
                                )
                            })
                            .collect();
                        return Err(format!(
                            "Expected diagnostic matching {:?}, but got:\n  {}",
                            pat,
                            actual_msgs.join("\n  ")
                        ));
                    }
                }
                return Ok(());
            }
        }
    }

    // Positive Test Assertion
    if directives.expect_accept || directives.expect_exit.is_some() {
        if let Err(diags) = check_res {
            let msgs: Vec<String> = diags
                .iter()
                .map(|d| {
                    format!(
                        "[{}] {}",
                        d.code.map(|c| c.to_string()).unwrap_or_else(|| "NO_CODE".to_string()),
                        d.message
                    )
                })
                .collect();
            return Err(format!(
                "Expected ACCEPT, but compilation FAILED with:\n  {}",
                msgs.join("\n  ")
            ));
        }

        // Native Execution Check if specified
        if let Some(expected_exit) = directives.expect_exit {
            let test_stem = file_path.file_stem().unwrap().to_string_lossy();
            let temp_dir = std::env::temp_dir()
                .join("luna_sem_stress")
                .join(test_stem.as_ref());
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();

            let exe_path = temp_dir.join(format!("{}.exe", test_stem));
            let compile_options = CompilerOptions {
                output_path: Some(exe_path.to_str().unwrap().to_string()),
                search_paths: vec![sysroot.root().to_string_lossy().to_string()],
                quiet: true,
                ..Default::default()
            };

            let compile_res = compile(file_path.to_str().unwrap(), source, &compile_options);
            if let Err(diags) = compile_res {
                let msgs: Vec<String> = diags.iter().map(|d| d.message.clone()).collect();
                return Err(format!("Native compile failed: {}", msgs.join("; ")));
            }

            let output = Command::new(&exe_path)
                .output()
                .map_err(|e| format!("Failed to execute {:?}: {}", exe_path, e))?;

            let actual_exit = output.status.code().unwrap_or(-1);
            if actual_exit != expected_exit {
                return Err(format!(
                    "Expected exit code {}, but program exited with {}",
                    expected_exit, actual_exit
                ));
            }

            if let Some(expected_stdout) = directives.expect_stdout {
                let actual_stdout = String::from_utf8_lossy(&output.stdout);
                if !actual_stdout.contains(&expected_stdout) {
                    return Err(format!(
                        "Expected stdout containing {:?}, but got {:?}",
                        expected_stdout, actual_stdout
                    ));
                }
            }
        }
    }

    Ok(())
}

fn collect_ln_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_ln_files(&path, acc);
            } else if path.extension().and_then(|s| s.to_str()) == Some("ln") {
                acc.push(path);
            }
        }
    }
}

#[test]
fn test_all_adversarial_semantic_stress_cases() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stress_root = manifest_dir.join("../../tests/sem_stress");

    if !stress_root.exists() {
        println!("Stress test directory {:?} does not exist yet.", stress_root);
        return;
    }

    let mut files = Vec::new();
    collect_ln_files(&stress_root, &mut files);
    files.sort();

    if files.is_empty() {
        println!("No stress test cases found under {:?}", stress_root);
        return;
    }

    let mut failures = Vec::new();
    let mut passed = 0;

    for file in &files {
        let rel_path = file.strip_prefix(&stress_root).unwrap_or(file);
        print!("Testing {:<60} ... ", rel_path.display());
        match run_stress_case(file) {
            Ok(()) => {
                println!("[OK]");
                passed += 1;
            }
            Err(msg) => {
                println!("[FAIL]");
                failures.push((rel_path.display().to_string(), msg));
            }
        }
    }

    if !failures.is_empty() {
        eprintln!("\n=================== STRESS FAILURES ===================");
        for (name, err) in &failures {
            eprintln!("\n[-] {}:\n{}", name, err);
        }
        eprintln!("=======================================================");
        panic!(
            "SEM-STRESS-01 failed: {}/{} passed, {} failed.",
            passed,
            files.len(),
            failures.len()
        );
    }

    println!("\nSEM-STRESS-01: All {} stress cases passed!", passed);
}
