use std::fs;
use std::path::{Path, PathBuf};
use mellis_driver::check;
use mellis_common::Diagnostic;

fn get_line_number(source: &str, byte_offset: u32) -> usize {
    source[..byte_offset as usize].chars().filter(|&c| c == '\n').count() + 1
}

fn run_ui_test(file_path: &Path) {
    let source = fs::read_to_string(file_path).expect("failed to read test file");
    
    let mut expected_errors = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        if let Some(idx) = line.find("//~ ERROR") {
            let msg = line[idx + "//~ ERROR".len()..].trim();
            expected_errors.push((line_idx + 1, msg.to_string()));
        }
    }
    
    let res = check(file_path.to_str().unwrap(), &source, true);
    let diagnostics = match res {
        Ok(()) => Vec::new(),
        Err(diags) => diags,
    };
    
    let mut actual_errors = Vec::new();
    for diag in diagnostics {
        let line_num = match diag.span {
            Some(span) => get_line_number(&source, span.start),
            None => 0, // 0 for spanless errors
        };
        actual_errors.push((line_num, diag.message.clone()));
    }
    
    let mut missing_errors = Vec::new();
    let mut matched_actuals = vec![false; actual_errors.len()];
    
    for (exp_line, exp_msg) in &expected_errors {
        let mut found = false;
        for (i, (act_line, act_msg)) in actual_errors.iter().enumerate() {
            if (*act_line == 0 || act_line == exp_line) && act_msg.contains(exp_msg) {
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
            unexpected_errors.push(format!("Line {}: {}", act_line, act_msg));
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
        panic!("{}", fail_msg);
    }
}

#[test]
fn e2e_ui_tests() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ui");
    if !tests_dir.exists() {
        return; // nothing to test
    }
    
    for entry in fs::read_dir(tests_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("ms") {
            println!("Testing: {}", path.display());
            run_ui_test(&path);
            println!("  [OK] {}", path.display());
        }
    }
}
