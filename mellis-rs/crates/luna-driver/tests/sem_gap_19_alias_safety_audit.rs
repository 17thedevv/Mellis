use luna_driver::sysroot::Sysroot;
use luna_driver::{check, CompilerOptions};
use std::fs;
use std::process::Command;

#[test]
fn test_place_model_investigation() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
fn test_model() -> i32 {
    dec rw x = 1;
    dec r = &rw x;
    dec y = *r;
    return y;
}
fn main() -> i32 {
    return test_model();
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        emit_mvir: true,
        ..Default::default()
    };
    let res = luna_driver::compile("test_model.ln", src.to_string(), &options);
    assert!(res.is_ok(), "test_model should compile: {:?}", res.err());
}

#[test]
fn test_negative_control_a() {
    // dec rw x = ...; dec r = &rw x; mutate x while r remains live; use r afterward
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
fn test_neg_a() -> i32 {
    dec rw x = 1;
    dec r = &rw x;
    x = 2;
    dec y = *r;
    y
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_neg_a.ln", src.to_string(), &options);
    assert!(res.is_err(), "Negative control A MUST be rejected!");
    let diags = res.err().unwrap();
    let has_borrow_conflict = diags.iter().any(|d| d.message.contains("borrowed as &rw") || d.message.contains("BorrowConflict"));
    assert!(has_borrow_conflict, "Expected borrow conflict diagnostic, got: {:?}", diags);
}

#[test]
fn test_negative_control_b() {
    // dec rw x = ...; dec r = &rw x; take another incompatible &rw x while r remains live
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
fn test_neg_b() -> i32 {
    dec rw x = 1;
    dec r = &rw x;
    dec r2 = &rw x;
    dec y = *r;
    dec y2 = *r2;
    y + y2
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_neg_b.ln", src.to_string(), &options);
    assert!(res.is_err(), "Negative control B MUST be rejected!");
    let diags = res.err().unwrap();
    let has_borrow_conflict = diags.iter().any(|d| d.message.contains("already borrowed as &rw") || d.message.contains("BorrowConflict"));
    assert!(has_borrow_conflict, "Expected borrow conflict diagnostic, got: {:?}", diags);
}

#[test]
fn test_negative_control_c() {
    // dec rw x = ...; dec r = &x; mutate x while r remains live; use r afterward
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
fn test_neg_c() -> i32 {
    dec rw x = 1;
    dec r = &x;
    x = 2;
    dec y = *r;
    y
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_neg_c.ln", src.to_string(), &options);
    assert!(res.is_err(), "Negative control C MUST be rejected!");
    let diags = res.err().unwrap();
    let has_borrow_conflict = diags.iter().any(|d| d.message.contains("borrowed as &") || d.message.contains("BorrowConflict"));
    assert!(has_borrow_conflict, "Expected borrow conflict diagnostic, got: {:?}", diags);
}

#[test]
fn test_negative_control_d() {
    // dec rw obj = ...; dec r = &rw obj.field; mutate the same obj.field while r remains live
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
struct Point {
    x: i32,
    y: i32,
}

fn test_neg_d() -> i32 {
    dec rw obj = Point { x: 1, y: 2 };
    dec r = &rw obj.x;
    obj.x = 42;
    dec y = *r;
    y
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_neg_d.ln", src.to_string(), &options);
    assert!(res.is_err(), "Negative control D MUST be rejected!");
    let diags = res.err().unwrap();
    let has_borrow_conflict = diags.iter().any(|d| d.message.contains("borrowed as &rw") || d.message.contains("BorrowConflict"));
    assert!(has_borrow_conflict, "Expected borrow conflict diagnostic, got: {:?}", diags);
}

#[test]
fn test_positive_control() {
    // Distinct allocas remain disjoint:
    // dec rw a = ...; dec rw b = ...; borrow a; mutate b; use borrow of a
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
fn test_pos() -> i32 {
    dec rw a = 1;
    dec rw b = 2;
    dec r = &rw a;
    b = 42;
    dec y = *r;
    y + b
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_pos.ln", src.to_string(), &options);
    assert!(res.is_ok(), "Positive control MUST compile cleanly! Got: {:?}", res.err());
}

#[test]
fn test_reborrow_control() {
    // r1 = &rw x; r2 = borrow/reborrow through r1; verify overlap with x is preserved
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src_conflict = r#"
fn test_reborrow_conflict() -> i32 {
    dec rw x = 1;
    dec r1 = &rw x;
    dec r2 = &rw *r1;
    x = 42; // MUST conflict because r2 is still live!
    dec y = *r2;
    y
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res_conflict = check("test_reborrow_conflict.ln", src_conflict.to_string(), &options);
    assert!(res_conflict.is_err(), "Reborrow conflict MUST be rejected!");
    let diags = res_conflict.err().unwrap();
    let has_borrow_conflict = diags.iter().any(|d| d.message.contains("borrowed as &rw") || d.message.contains("BorrowConflict"));
    assert!(has_borrow_conflict, "Expected borrow conflict diagnostic on reborrow, got: {:?}", diags);

    // Valid reborrow without mutation of x
    let src_valid = r#"
fn test_reborrow_valid() -> i32 {
    dec rw x = 1;
    dec r1 = &rw x;
    dec r2 = &rw *r1;
    *r2 = 42;
    dec y = *r2;
    y
}
"#;
    let res_valid = check("test_reborrow_valid.ln", src_valid.to_string(), &options);
    assert!(res_valid.is_ok(), "Valid reborrow MUST compile! Got: {:?}", res_valid.err());
}

#[test]
fn test_method_call_genuine_conflict_control() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
struct Container {
    val: i32,
}

impl Container {
    fn mutate_with(self: &rw Self, arg: &rw i32) {
        self.val = self.val + *arg;
    }
}

fn test_conflict() {
    dec rw c = Container { val: 10 };
    c.mutate_with(&rw c.val);
}
"#;
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let res = check("test_method_conflict.ln", src.to_string(), &options);
    assert!(res.is_err(), "Method call with argument borrowing receiver field MUST be rejected with conflict!");
    let diags = res.err().unwrap();
    println!("Method genuine conflict diags: {:?}", diags);
    let has_conflict = diags.iter().any(|d| d.message.contains("borrow") || d.message.contains("BorrowConflict"));
    assert!(has_conflict, "Expected borrow conflict diagnostic, got: {:?}", diags);
}

#[test]
fn test_evaluation_order_sequence() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
struct OrderTracker {
    seq: i32,
}

impl OrderTracker {
    fn step(self: &rw Self, val: i32) -> i32 {
        self.seq = self.seq * 10 + val;
        val
    }
}

struct Target {
    dummy: i32,
}

impl Target {
    fn run_mut(self: &rw Self, a: i32, b: i32) -> i32 {
        self.dummy + a + b
    }
    fn run_ref(self: &Self, a: i32, b: i32) -> i32 {
        self.dummy + a + b
    }
    fn run_val(self: Self, a: i32, b: i32) -> i32 {
        self.dummy + a + b
    }
}

fn get_target(tracker: &rw OrderTracker, step_num: i32) -> Target {
    tracker.step(step_num);
    Target { dummy: 1000 }
}

fn main() -> i32 {
    dec rw tracker = OrderTracker { seq: 0 };
    
    // Test 1: &rw self method call
    // Evaluation MUST be: receiver (step 1) -> arg1 (step 2) -> arg2 (step 3)
    dec rw t = get_target(&rw tracker, 1);
    t.run_mut(tracker.step(2), tracker.step(3));
    
    // seq must be 123 (1 * 10 + 2 = 12; 12 * 10 + 3 = 123)
    if tracker.seq != 123 {
        return 1;
    }
    
    // Test 2: chained receiver expression evaluated exactly once:
    // get_target(&rw tracker, 4).run_mut(...)
    // Sequence continues: step 4 (receiver) -> step 5 (arg1) -> step 6 (arg2)
    get_target(&rw tracker, 4).run_mut(tracker.step(5), tracker.step(6));
    // seq was 123; then 1234 -> 12345 -> 123456
    if tracker.seq != 123456 {
        return 2;
    }

    // Test 3: &self method call
    dec rw t2 = Target { dummy: 1000 };
    t2.run_ref(tracker.step(7), tracker.step(8));
    if tracker.seq != 12345678 {
        return 3;
    }

    // Test 4: by-value self method call
    t2.run_val(tracker.step(9), 0);
    // seq ends with 9
    if tracker.seq != 123456789 {
        return 4;
    }

    // Test 5: already-reference receiver
    dec rw t3 = Target { dummy: 2000 };
    dec r = &rw t3;
    r.run_mut(1, 2);

    return 0;
}
"#;
    let temp = std::env::temp_dir().join("sem_gap_19_eval_order");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    let exe_path = temp.join("test.exe");

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = luna_driver::compile("eval_order.ln", src.to_string(), &options);
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());

    let output = Command::new(&exe_path).output().expect("Failed to run");
    let exit_code = output.status.code().unwrap_or(-1);
    assert_eq!(exit_code, 0, "Execution failed with exit code: {}", exit_code);
}

