use std::fs;
use std::path::PathBuf;
use mellis_driver::check;

#[test]
fn test_drop_reassign_in_branch_passes() {
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui")
        .join("drop_reassign_in_branch.ms");
    let source = fs::read_to_string(&file_path).expect("failed to read test file");
    
    let res = check(file_path.to_str().unwrap(), source.clone(), &[], true);
    match res {
        Ok(()) => {
            println!("PASS: check() returned Ok — no diagnostics emitted.");
            println!("ConditionallyMoved does NOT reject move-then-reassign-in-same-branch pattern.");
        }
        Err(diags) => {
            let mut fail_msg = format!("FAIL: drop_reassign_in_branch.ms was rejected with {} diagnostics:\n", diags.len());
            for d in &diags {
                fail_msg.push_str(&format!("  [{:?}] span={:?} msg={}\n", d.level, d.span, d.message));
            }
            panic!("{}", fail_msg);
        }
    }
}
