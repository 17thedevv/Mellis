use luna_driver::sysroot::Sysroot;
use luna_driver::{check, CompilerOptions};

fn test_check(name: &str, src: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    check(name, src.to_string(), &options)
}

// ----------------------------------------------------------------------------
// SEM-GAP-17 Phase 2D-B Conformance & Regression Test Suite
// ----------------------------------------------------------------------------

#[test]
fn sem_loop_01_outer_ref_reassigned_in_loop() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw y = 20;
            dec rw r = &x;
            dec rw i = 0;
            while i < 3 {
                r = &y;
                x = 5;
                i = i + 1;
            }
            return *r;
        }
    "#;
    let res = test_check("sem_loop_01", src);
    assert!(res.is_ok(), "Outer ref reassigned in loop should allow referent write before assignment: {:?}", res.err());
}

#[test]
fn sem_loop_02_mut_borrow_mutated_at_top_of_loop() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                x = x + 1;
                dec r = &rw x;
                *r = *r + 2;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_02", src);
    assert!(res.is_ok(), "Mut borrow at bottom should not conflict with mutation at top of next iteration: {:?}", res.err());
}

#[test]
fn sem_loop_03_mut_borrow_mutated_after_in_same_iteration() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                dec r = &rw x;
                *r = *r + 2;
                x = x + 1;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_03", src);
    assert!(res.is_ok(), "Mutating referent after mut borrow ends in same iteration must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_04_iteration_local_mut_borrowed() {
    let src = r#"
        fn main() -> i32 {
            dec rw i = 0;
            while i < 3 {
                dec rw v = i;
                dec r = &rw v;
                *r = *r + 1;
                i = i + 1;
            }
            return 0;
        }
    "#;
    let res = test_check("sem_loop_04", src);
    assert!(res.is_ok(), "Iteration-local variable mutably borrowed per iteration must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_05_iteration_local_assigned_to_outer_ref_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw r = &x;
            dec rw i = 0;
            while i < 3 {
                dec rw v = i;
                r = &v;
                i = i + 1;
            }
            return *r;
        }
    "#;
    let res = test_check("sem_loop_05", src);
    assert!(res.is_err(), "Borrow of iteration-local assigned to outer reference must be rejected");
    let diags = res.err().unwrap();
    let has_escape = diags.iter().any(|d| d.message.contains("E3005") && d.message.contains("escapes loop iteration"));
    assert!(has_escape, "Expected E3005 LocalBorrowEscape across loop iteration, got: {:?}", diags);
}

#[test]
fn sem_loop_06_shared_then_mut_across_iterations() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                dec r = &x;
                dec val = *r;
                dec r2 = &rw x;
                *r2 = val + 1;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_06", src);
    assert!(res.is_ok(), "Shared borrow ended before mut borrow in loop iteration must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_07_outer_mut_ref_alternating_assignments() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw y = 20;
            dec rw r = &rw x;
            dec rw i = 0;
            while i < 3 {
                *r = *r + 1;
                r = &rw y;
                *r = *r + 1;
                r = &rw x;
                i = i + 1;
            }
            return x + y;
        }
    "#;
    let res = test_check("sem_loop_07", src);
    assert!(res.is_ok(), "Alternating strong updates to outer mut ref in loop must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_08_outer_mut_ref_dead_across_back_edge() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw r = &rw x;
            *r = 20;
            dec rw i = 0;
            while i < 3 {
                x = x + 1;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_08", src);
    assert!(res.is_ok(), "Dead outer mut ref must not block mutation in loop: {:?}", res.err());
}

#[test]
fn sem_loop_09_reborrow_in_conditional_loop() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                if i == 0 {
                    dec r = &rw x;
                    *r = 20;
                } else {
                    x = x + 1;
                }
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_09", src);
    assert!(res.is_ok(), "Conditional borrow in loop dead before other branch must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_10_loan_live_only_on_break() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                dec r = &rw x;
                if *r > 10 {
                    break;
                }
                *r = *r + 1;
                i = i + 1;
            }
            x = 30;
            return x;
        }
    "#;
    let res = test_check("sem_loop_10", src);
    assert!(res.is_ok(), "Loan live only on break must not block post-loop mutation if loop exits: {:?}", res.err());
}

#[test]
fn sem_loop_11_iteration_local_struct_carrying_ref() {
    let src = r#"
        struct Holder {
            r: &i32,
        };
        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                dec h = Holder { r: &x };
                dec v = *h.r;
                x = x + 1;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_loop_11", src);
    assert!(res.is_ok(), "Iteration-local struct carrying borrow dead before mutation must succeed: {:?}", res.err());
}

// ----------------------------------------------------------------------------
// Lifetime-Region Exit Audit (SEM-GAP-17 Model Generalization)
// ----------------------------------------------------------------------------

#[test]
fn sem_loop_12_escape_via_break_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec rw outer_ref: &i32 = &0;
            dec rw i = 0;
            while i < 5 {
                dec local_val = 42;
                outer_ref = &local_val;
                break;
            }
            return *outer_ref;
        }
    "#;
    let res = test_check("sem_loop_12", src);
    assert!(res.is_err(), "Local borrow escaping loop via break must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("error[E3005]") || d.message.contains("escapes")),
        "Expected error[E3005], got: {:?}",
        errs
    );
}

#[test]
fn sem_loop_13_escape_via_continue_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec rw outer_ref: &i32 = &0;
            dec rw i = 0;
            while i < 5 {
                dec local_val = 42;
                outer_ref = &local_val;
                i = i + 1;
                continue;
            }
            return *outer_ref;
        }
    "#;
    let res = test_check("sem_loop_13", src);
    assert!(res.is_err(), "Local borrow escaping to outer carrier across continue must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("error[E3005]") || d.message.contains("escapes")),
        "Expected error[E3005], got: {:?}",
        errs
    );
}

#[test]
fn sem_loop_14_non_escaping_temporary_survives_continue() {
    let src = r#"
        fn main() -> i32 {
            dec rw sum = 0;
            dec rw i = 0;
            while i < 5 {
                dec rw local_val = i * 2;
                dec r = &local_val;
                sum = sum + *r;
                i = i + 1;
                continue;
            }
            return sum;
        }
    "#;
    let res = test_check("sem_loop_14", src);
    assert!(res.is_ok(), "Non-escaping temporary across continue must succeed: {:?}", res.err());
}

#[test]
fn sem_loop_15_escape_via_normal_loop_exit_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec rw outer_ref: &i32 = &0;
            dec rw i = 0;
            while i < 3 {
                dec local_val = 100 + i;
                outer_ref = &local_val;
                i = i + 1;
            }
            return *outer_ref;
        }
    "#;
    let res = test_check("sem_loop_15", src);
    assert!(res.is_err(), "Local borrow escaping loop via normal loop condition exit must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("error[E3005]") || d.message.contains("escapes")),
        "Expected error[E3005], got: {:?}",
        errs
    );
}

#[test]
fn sem_loop_16_nested_loop_inner_local_dies_outer_loan_survives() {
    let src = r#"
        fn main() -> i32 {
            dec rw outer_val = 10;
            dec rw i = 0;
            while i < 2 {
                dec outer_ref = &rw outer_val;
                dec rw j = 0;
                while j < 2 {
                    dec rw inner_val = 20;
                    dec inner_ref = &rw inner_val;
                    *inner_ref = *inner_ref + 1;
                    j = j + 1;
                }
                *outer_ref = *outer_ref + 1;
                i = i + 1;
            }
            return outer_val;
        }
    "#;
    let res = test_check("sem_loop_16", src);
    assert!(res.is_ok(), "Inner loop local deaths must not kill outer loop loan: {:?}", res.err());
}

#[test]
fn sem_loop_17_nested_loop_inner_borrow_escaping_to_outer_rejected() {
    let src = r#"
        fn main() -> i32 {
            dec rw outer_carrier: &i32 = &0;
            dec rw i = 0;
            while i < 2 {
                dec rw j = 0;
                while j < 2 {
                    dec inner_val = 99;
                    outer_carrier = &inner_val;
                    j = j + 1;
                }
                dec read_outer = *outer_carrier;
                i = i + 1;
            }
            return 0;
        }
    "#;
    let res = test_check("sem_loop_17", src);
    assert!(res.is_err(), "Inner loop local borrow escaping to outer carrier must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("error[E3005]") || d.message.contains("escapes")),
        "Expected error[E3005], got: {:?}",
        errs
    );
}

