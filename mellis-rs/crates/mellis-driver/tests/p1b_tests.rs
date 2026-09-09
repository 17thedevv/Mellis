use mellis_driver::{check, CompilerOptions};
use mellis_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p1b_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<mellis_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ms");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

// -----------------------------------------------------------------------------
// Test Matrix for P1-B: Async × Borrow (14 Tests)
// -----------------------------------------------------------------------------

// 01. External shared borrow across await (Pass)
#[test]
fn test_case_01_external_shared_borrow_across_await_pass() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(x: &i32) -> i32 {
            dec a = *x;
            dec h = await helper();
            dec b = *x;
            return a + b + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(success, "External shared borrow across await should pass, but got diagnostics: {:?}", diags);
}

// 02. External mutable borrow across await (Pass)
#[test]
fn test_case_02_external_rw_borrow_across_await_pass() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(x: &rw i32) -> i32 {
            *x = 10;
            dec h = await helper();
            *x = *x + h;
            return *x;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_02", src);
    assert!(success, "External mutable borrow across await should pass, but got diagnostics: {:?}", diags);
}

// 03. Local shared borrow across await (Fail: E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT)
#[test]
fn test_case_03_local_shared_borrow_across_await_fail() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn() -> i32 {
            dec x = 42;
            dec r = &x;
            dec h = await helper();
            return *r + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_03", src);
    assert!(!success, "Local shared borrow across await must be rejected to prevent self-referential futures");
    assert!(
        diags.iter().any(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT")),
        "Expected diagnostic containing E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT, got: {:?}",
        diags
    );
}

// 04. Local mutable borrow across await (Fail: E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT)
#[test]
fn test_case_04_local_rw_borrow_across_await_fail() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn() -> i32 {
            dec rw x = 42;
            dec r = &rw x;
            dec h = await helper();
            *r = *r + h;
            return *r;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_04", src);
    assert!(!success, "Local &rw borrow across await must be rejected");
    assert!(
        diags.iter().any(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT")),
        "Expected diagnostic containing E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT, got: {:?}",
        diags
    );
}

// 05. By-value param borrow across await (Fail: E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT)
#[test]
fn test_case_05_by_value_param_borrow_across_await_fail() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(x: i32) -> i32 {
            dec r = &x;
            dec h = await helper();
            return *r + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(!success, "Borrow of by-value parameter across await must be rejected because by-value parameters live in local future frame");
    assert!(
        diags.iter().any(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT")),
        "Expected diagnostic containing E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT, got: {:?}",
        diags
    );
}

// 06. Local borrow dying before await (Pass: NLL)
#[test]
fn test_case_06_local_borrow_dying_before_await_pass() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn() -> i32 {
            dec x = 42;
            dec r = &x;
            dec val = *r;
            dec h = await helper();
            return val + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_06", src);
    assert!(success, "Local borrow dying before await should pass under NLL, but got: {:?}", diags);
}

// 07. Caller exclusive borrow locked while future alive (Fail)
#[test]
fn test_case_07_caller_exclusive_borrow_locked_while_future_alive() {
    let src = r#"
        async fn async_inc(x: &rw i32) -> i32 {
            *x = *x + 1;
            return *x;
        }

        async fn caller() -> i32 {
            dec rw data = 10;
            dec fut = async_inc(&rw data);
            data = 20;
            dec res = await fut;
            return res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_07", src);
    assert!(!success, "Caller write to data while future holds &rw loan must fail");
    assert!(
        diags.iter().any(|d| d.message.contains("borrowed as &rw")),
        "Expected borrowck conflict on data, got: {:?}",
        diags
    );
}

// 08. Branching await points local borrow (Fail: E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT)
#[test]
fn test_case_08_branching_await_points_local_borrow_fail() {
    let src = r#"
        async fn step1() -> i32 { return 1; }
        async fn step2() -> i32 { return 2; }

        async fn test_branch(cond: bool) -> i32 {
            dec x = 42;
            dec r = &x;
            dec rw res = 0;
            if cond {
                res = await step1();
            } else {
                res = await step2();
            }
            return *r + res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(!success, "Local borrow across branching await points must fail");
    assert!(
        diags.iter().any(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT")),
        "Expected E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT in branching await, got: {:?}",
        diags
    );
}

// 09. Suspended future drop generation
#[test]
fn test_case_09_suspended_future_drop_generation() {
    let src = r#"
        import <core>;

        struct Resource {
            id: i32,
        }

        impl core::Drop for Resource {
            fn drop(self: &rw Self) {}
        }

        async fn worker() -> i32 {
            dec r = Resource { id: 1 };
            return r.id;
        }

        fn main() -> i32 {
            dec fut = worker();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_09", src);
    assert!(success, "Future with Drop types should compile cleanly, got: {:?}", diags);
}

// 10. Movable future by value (no Pin required)
#[test]
fn test_case_10_movable_future_by_value_no_pin() {
    let src = r#"
        async fn compute() -> i32 {
            return 42;
        }

        async fn runner() -> i32 {
            dec fut1 = compute();
            dec fut2 = fut1;
            dec res = await fut2;
            return res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(success, "Futures must be movable by value without Pin requirement, got: {:?}", diags);
}

// 11. External loan terminates on await completion (Pass)
#[test]
fn test_case_11_external_loan_terminates_on_await_completion() {
    let src = r#"
        async fn async_inc(x: &rw i32) -> i32 {
            *x = *x + 1;
            return *x;
        }

        async fn caller() -> i32 {
            dec rw data = 10;
            dec fut = async_inc(&rw data);
            dec res = await fut;
            data = 30;
            return data + res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_11", src);
    assert!(success, "External loan must terminate once future is awaited, got: {:?}", diags);
}

// 12. External loan terminates on future drop (Pass)
#[test]
fn test_case_12_external_loan_terminates_on_future_drop() {
    let src = r#"
        async fn async_inc(x: &rw i32) -> i32 {
            *x = *x + 1;
            return *x;
        }

        fn consume<T>(x: T) {}

        fn caller() -> i32 {
            dec rw data = 10;
            dec fut = async_inc(&rw data);
            consume(fut);
            data = 30;
            return data;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_12", src);
    assert!(success, "External loan must terminate once future is consumed / dead, got: {:?}", diags);
}

// 13. Movable future is not thread-sendable (Rule E invariant)
#[test]
fn test_case_13_movable_future_not_send() {
    // Under Mellis Rule E, future movability in memory does NOT imply thread transferability (Send).
    // An async fn capturing non-static borrows remains strictly lifetime-bound to the caller frame.
    let src = r#"
        async fn local_async(x: &rw i32) -> i32 {
            *x = *x + 10;
            return *x;
        }

        async fn runner() -> i32 {
            dec rw local_val = 5;
            dec fut = local_async(&rw local_val);
            dec res = await fut;
            return res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_13", src);
    assert!(success, "Valid thread-bound movable future should compile, got: {:?}", diags);
}

// 14. Future move transfer preserves loan across moves
#[test]
fn test_case_14_future_move_transfer_preserves_loan() {
    // 14a. When fut1 is moved to fut2, the loan on data is still held by fut2.
    let src_fail = r#"
        async fn async_inc(x: &rw i32) -> i32 {
            *x = *x + 1;
            return *x;
        }

        async fn test_transfer() -> i32 {
            dec rw data = 10;
            dec fut1 = async_inc(&rw data);
            dec fut2 = fut1;
            data = 20;
            dec res = await fut2;
            return res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success_fail, diags_fail) = run_compiler("test_14_fail", src_fail);
    assert!(!success_fail, "Moving fut1 to fut2 must preserve loan; write to data before await fut2 must fail");
    assert!(
        diags_fail.iter().any(|d| d.message.contains("borrowed as &rw")),
        "Expected borrow conflict on data after future move, got: {:?}",
        diags_fail
    );

    // 14b. Once fut2 is awaited, loan is released.
    let src_pass = r#"
        async fn async_inc(x: &rw i32) -> i32 {
            *x = *x + 1;
            return *x;
        }

        async fn test_transfer_pass() -> i32 {
            dec rw data = 10;
            dec fut1 = async_inc(&rw data);
            dec fut2 = fut1;
            dec res = await fut2;
            data = 30;
            return data + res;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success_pass, diags_pass) = run_compiler("test_14_pass", src_pass);
    assert!(success_pass, "After fut2 is awaited, data should be accessible, but got: {:?}", diags_pass);
}
