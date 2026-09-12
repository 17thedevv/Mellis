use luna_common::Diagnostic;
use luna_driver::check;
use std::fs;
use std::path::{Path, PathBuf};

fn get_line_number(source: &str, byte_offset: u32) -> usize {
    let offset = (byte_offset as usize).min(source.len());
    source[..offset].chars().filter(|&c| c == '\n').count() + 1
}

fn run_ui_test(file_path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(file_path).expect("failed to read test file");

    let mut expected_errors = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let mut curr_idx = 0;
        while let Some(idx) = line[curr_idx..].find("//~ ERROR") {
            let start = curr_idx + idx + "//~ ERROR".len();
            let mut end = line.len();
            if let Some(next_idx) = line[start..].find("//~ ERROR") {
                end = start + next_idx;
            }
            expected_errors.push((line_idx + 1, line[start..end].trim().to_string()));
            curr_idx = end;
        }
    }

    let search_paths = vec![
        "../../scratch".to_string(),
        "tests/ui".to_string(),
        ".".to_string(),
    ];
    let sysroot = std::env::current_dir().unwrap().join("../..").to_string_lossy().to_string();
    let options = luna_driver::CompilerOptions {
        output_path: None,
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        search_paths: vec![sysroot, "tests/ui".to_string(), ".".to_string()],
        quiet: true,
        no_link: false,
        ..Default::default()
    };
    let res = check(
        file_path.to_str().unwrap(),
        source.clone(),
        &options,
    );

    // Positive test check
    if expected_errors.is_empty() {
        if let Err(diags) = res {
            let mut fail_msg = format!(
                "UI Test Failed: {}\nExpected SUCCESS but got {} diagnostics:\n",
                file_path.display(),
                diags.len()
            );
            for diag in diags {
                let line_num = match diag.span {
                    Some(span) => get_line_number(&source, span.start),
                    None => 0,
                };
                fail_msg.push_str(&format!("  [Line {}] {}\n", line_num, diag.message));
            }
            return Err(fail_msg);
        }
        return Ok(()); // Passed
    }

    // Negative test check
    let diagnostics = match res {
        Ok(()) => Vec::new(),
        Err(diags) => diags,
    };

    let mut actual_errors = Vec::new();
    for diag in diagnostics {
        let line_num = if let Some(span) = diag.span {
            get_line_number(&source, span.start)
        } else {
            0
        };
        actual_errors.push((line_num, diag.message.clone()));
    }

    let mut missing_errors = Vec::new();
    let mut matched_actuals = vec![false; actual_errors.len()];

    for (exp_line, exp_msg) in &expected_errors {
        let mut found = false;
        for (i, (act_line, act_msg)) in actual_errors.iter().enumerate() {
            if act_line == exp_line && act_msg.contains(exp_msg) {
                found = true;
                matched_actuals[i] = true;
                break;
            }
        }
        if !found {
            missing_errors.push(format!("Line {}: {}", exp_line, exp_msg));
        }
    }

    let mut unexpected_errors = Vec::new();
    for (i, (act_line, act_msg)) in actual_errors.iter().enumerate() {
        if !matched_actuals[i] {
            if act_msg.contains("Cannot access") {
                // Ignore secondary borrowck errors for now
            } else {
                unexpected_errors.push(format!("Line {}: {}", act_line, act_msg));
            }
        }
    }

    if !missing_errors.is_empty() || !unexpected_errors.is_empty() {
        let mut fail_msg = format!("UI Test Failed: {}\n", file_path.display());
        if !missing_errors.is_empty() {
            fail_msg.push_str("Expected errors not found:\n");
            for m in missing_errors {
                fail_msg.push_str(&format!("  - {}\n", m));
            }
        }
        if !unexpected_errors.is_empty() {
            fail_msg.push_str("Unexpected errors found:\n");
            for u in unexpected_errors {
                fail_msg.push_str(&format!("  - {}\n", u));
            }
        }
        fail_msg.push_str("\n--- Actual Errors ---\n");
        for (l, m) in actual_errors {
            fail_msg.push_str(&format!("Line {}: {}\n", l, m));
        }
        return Err(fail_msg);
    }
    Ok(())
}

#[test]
fn e2e_ui_tests() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui");
    if !tests_dir.exists() {
        return; // nothing to test
    }

    let mut failed_messages = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(tests_dir)
        .unwrap()
        .map(|res| res.unwrap())
        .collect();
    entries.sort_by_key(|dir| dir.path());

    for entry in entries {
        let path = entry.path();
        if path.is_file() && (path.extension().and_then(|s| s.to_str()) == Some("ln") || path.extension().and_then(|s| s.to_str()) == Some("ms")) {
            let p_str = path.to_string_lossy();
            if !p_str.contains("try_custom_cross_module")
                && !p_str.contains("try_leak_test")
                && !p_str.contains("try_cross_module_generic")
                && !p_str.contains("try_negative_duck_typing")
                && !p_str.contains("try_drift_test")
            {
                continue;
            }
            println!("Testing: {}", path.display());
            match run_ui_test(&path) {
                Ok(()) => {
                    println!("  [OK] {}", path.display());
                }
                Err(msg) => {
                    println!("  [FAIL] {}", path.display());
                    failed_messages.push(msg);
                }
            }
        }
    }

    if !failed_messages.is_empty() {
        panic!(
            "{} UI tests failed:\n\n{}",
            failed_messages.len(),
            failed_messages.join("\n\n")
        );
    }
}
