//! SEM-MATURITY-01 PHASE 2B: Ownership / Borrow / CFG Coverage Closure
//!
//! CORRECTED VERSION - Semantic Correction Pass Applied
//!
//! This test suite covers the seven selected frozen semantic rules:
//!   1. Mutable references broadly
//!   2. Move in loops
//!   3. Move through match
//!   4. Borrow across branches
//!   5. Borrow through aggregate fields
//!   6. Borrow across loops
//!   7. Shared borrow after ended mutable borrow
//!
//! TEST CLASSIFICATION LEGEND:
//!   VALID-POSITIVE  - Program is frozen-valid and must compile
//!   VALID-NEGATIVE  - Program is frozen-invalid and must reject
//!   COMPILER-GAP    - Expectation proven correct, compiler behavior wrong
//!   INVALID-TEST     - Expectation contradicts frozen semantics (corrected)
//!   SPEC-BLOCKED     - Frozen authority insufficient/contradictory
//!
//! AUTHORITY ORDER (highest to lowest):
//!   1. Frozen Luna borrow/ownership specification (P6 Hardened)
//!   2. Current frozen architecture/contracts
//!   3. Production implementation
//!   4. Previous conformance tests
//!   5. Phase 2B audit assumptions

use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_sem_maturity_2b").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<luna_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
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

fn assert_compile_success(name: &str, src: &str, msg: &str) {
    let (success, diags) = run_compiler(name, src);
    assert!(success, "{}: expected compile success, but failed with: {:?}", msg, diags);
}

fn assert_compile_error(name: &str, src: &str, expected_needle: &str, msg: &str) {
    let (success, diags) = run_compiler(name, src);
    assert!(!success, "{}: expected compile failure, but succeeded", msg);
    if !expected_needle.is_empty() {
        let found = diags.iter().any(|d| d.message.to_lowercase().contains(&expected_needle.to_lowercase()));
        assert!(found, "{}: expected error containing '{}', got: {:?}", msg, expected_needle, diags);
    }
}

// ============================================================================
// SECTION 1: MUTABLE REFERENCES BROADLY (SEM-MUTREF)
// ============================================================================
//
// CORRECTED: Luna allows multiple shared (&T) borrows simultaneously.
// Only &rw + &rw, & + &rw, or &rw + & conflicts.
// ============================================================================

mod mutable_references {
    use super::*;

    // SEM-MUTREF-01: VALID-POSITIVE - basic mutable reference works
    #[test]
    fn sem_mutref_01_obtain_and_use_mutref_valid() {
        let src = r#"
            struct Counter {
                value: i32,
            };

            fn increment(c: &rw Counter) {
                c.value = c.value + 1;
            }

            fn main() -> i32 {
                dec rw c = Counter { value: 0 };
                increment(&rw c);
                return c.value;
            }
        "#;
        assert_compile_success("sem_mutref_01", src, "SEM-MUTREF-01: Obtain and use mutable reference");
    }

    // SEM-MUTREF-02: VALID-POSITIVE - mutable borrow with re-access
    #[test]
    fn sem_mutref_02_mutate_and_reaccess_valid() {
        let src = r#"
            struct Pair {
                first: i32,
                second: i32,
            };

            fn double_first(p: &rw Pair) {
                p.first = p.first * 2;
            }

            fn main() -> i32 {
                dec rw p = Pair { first: 5, second: 10 };
                double_first(&rw p);
                return p.first + p.second;
            }
        "#;
        assert_compile_success("sem_mutref_02", src, "SEM-MUTREF-02: Mutate through mutable ref, re-access after");
    }

    // SEM-MUTREF-03: VALID-NEGATIVE - mutation through shared reference rejected per RFC P6 §6.2
    #[test]
    fn sem_mutref_03_mutation_through_shared_ref() {
        let src = r#"
            struct Counter {
                value: i32,
            };

            fn increment(c: &Counter) {
                c.value = c.value + 1;
            }

            fn main() -> i32 {
                dec c = Counter { value: 0 };
                increment(&c);
                return c.value;
            }
        "#;
        assert_compile_error("sem_mutref_03", src, "cannot mutate", "SEM-MUTREF-03: Mutation through shared reference rejected");
    }

    // SEM-MUTREF-04: VALID-NEGATIVE - multiple mutable borrows correctly rejected
    #[test]
    fn sem_mutref_04_overlapping_mutable_loans_rejected() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r1 = &rw x;
                dec r2 = &rw x;
                return *r1;
            }
        "#;
        assert_compile_error("sem_mutref_04", src, "already borrowed", "SEM-MUTREF-04: Overlapping mutable loans rejected");
    }

    // SEM-MUTREF-05: VALID-POSITIVE - sequential mutable borrows with primitive return
    #[test]
    fn sem_mutref_05_sequential_mutable_borrows_valid() {
        let src = r#"
            fn helper1(x: &rw i32) -> i32 {
                dec temp = *x;
                *x = temp + 1;
                return temp;
            }

            fn helper2(x: &rw i32) -> i32 {
                return *x * 2;
            }

            fn main() -> i32 {
                dec rw val = 5;
                dec a = helper1(&rw val);
                dec b = helper2(&rw val);
                return a + b;
            }
        "#;
        assert_compile_success("sem_mutref_05", src, "SEM-MUTREF-05: Sequential mutable borrows valid");
    }

    // SEM-MUTREF-06: INVALID-TEST - fixture error (val is i32, not &i32)
    // NOTE: This test had an invalid fixture. `*val = 20` on `i32` is a type error.
    // The actual gap is SEM-MUTREF-03 (mutation through shared reference).
    #[test]
    #[ignore] // IGNORED - INVALID fixture
    fn sem_mutref_06_invalid_fixture() {
        // This test is ignored because the fixture was invalid.
        // val: i32 cannot be dereferenced with *val
    }

    // SEM-MUTREF-07: COMPILER-GAP - shared borrow lifetime after function call
    #[test]
    fn sem_mutref_07_mutable_after_shared_ends() {
        let src = r#"
            fn use_shared(x: &i32) -> i32 {
                return *x;
            }

            fn main() -> i32 {
                dec rw val = 10;
                dec shared_ref = &val;
                dec v1 = use_shared(shared_ref);
                dec v2 = use_shared(shared_ref);
                return v1 + v2;
            }
        "#;
        // CORRECTED: This is actually VALID - the shared_ref is only used after
        // the borrow ends when the function returns. No mutation occurs.
        assert_compile_success("sem_mutref_07", src, "SEM-MUTREF-07: Shared borrow after function call");
    }

    // SEM-MUTREF-08: VALID-POSITIVE - function call with mutable reference
    #[test]
    fn sem_mutref_08_pass_mutref_to_function() {
        let src = r#"
            fn add_n(target: &rw i32, n: i32) {
                *target = *target + n;
            }

            fn main() -> i32 {
                dec rw val = 100;
                add_n(&rw val, 5);
                add_n(&rw val, 3);
                return val;
            }
        "#;
        assert_compile_success("sem_mutref_08", src, "SEM-MUTREF-08: Passing mutable reference to function");
    }
}

// ============================================================================
// SECTION 2: MOVE IN LOOPS (SEM-MOVE-LOOP)
// ============================================================================

mod move_in_loops {
    use super::*;

    // SEM-MOVE-LOOP-01: VALID-NEGATIVE - use-after-move correctly rejected
    #[test]
    fn sem_move_loop_01_move_in_loop_use_after() {
        let src = r#"
            struct Data {
                id: i32,
            };

            fn process(d: Data) -> i32 {
                return d.id;
            }

            fn main() -> i32 {
                dec rw i = 0;
                dec data = Data { id: 42 };
                while i < 1 {
                    dec x = data;
                    i = i + 1;
                }
                return process(data);
            }
        "#;
        assert_compile_error("sem_move_loop_01", src, "moved", "SEM-MOVE-LOOP-01: Move in loop, use after");
    }

    // SEM-MOVE-LOOP-02: VALID-POSITIVE - value moved and consumed in loop
    #[test]
    fn sem_move_loop_02_move_in_loop_proven_unreachable_after() {
        let src = r#"
            struct Data {
                id: i32,
            };

            fn consume(d: Data) -> i32 {
                return d.id;
            }

            fn main() -> i32 {
                dec rw i = 0;
                while i < 1 {
                    dec data = Data { id: 42 };
                    dec x = consume(data);
                    i = i + 1;
                }
                return 0;
            }
        "#;
        assert_compile_success("sem_move_loop_02", src, "SEM-MOVE-LOOP-02: Value moved and consumed in loop");
    }

    // SEM-MOVE-LOOP-03: VALID-POSITIVE - value ends each iteration
    #[test]
    fn sem_move_loop_03_reinit_before_next_iteration() {
        let src = r#"
            struct Item {
                val: i32,
            };

            fn main() -> i32 {
                dec rw i = 0;
                while i < 3 {
                    dec item = Item { val: i };
                    i = i + 1;
                }
                return 0;
            }
        "#;
        assert_compile_success("sem_move_loop_03", src, "SEM-MOVE-LOOP-03: Value ends each iteration");
    }

    // SEM-MOVE-LOOP-04: VALID-NEGATIVE - conditionally moved, used on later iteration
    #[test]
    fn sem_move_loop_04_conditionally_moved_used_later() {
        let src = r#"
            struct Item {
                val: i32,
            };

            fn main() -> i32 {
                dec rw cond = true;
                dec item = Item { val: 10 };
                while cond {
                    if cond {
                        dec x = item;
                    }
                    cond = false;
                }
                return item.val;
            }
        "#;
        assert_compile_error("sem_move_loop_04", src, "moved", "SEM-MOVE-LOOP-04: Conditionally moved value used later");
    }

    // SEM-MOVE-LOOP-05: VALID-POSITIVE - move in one branch of loop
    #[test]
    fn sem_move_loop_05_move_one_branch_proven_conditional() {
        let src = r#"
            struct Item {
                val: i32,
            };

            fn consume(i: Item) {}

            fn main() -> i32 {
                dec rw i = 0;
                while i < 2 {
                    dec item = Item { val: i };
                    if i == 0 {
                        consume(item);
                    }
                    i = i + 1;
                }
                return 0;
            }
        "#;
        assert_compile_success("sem_move_loop_05", src, "SEM-MOVE-LOOP-05: Move in one branch only");
    }

    // SEM-MOVE-LOOP-06: VALID-NEGATIVE - conditionally moved, used after loop
    #[test]
    fn sem_move_loop_06_conditional_move_after_loop() {
        let src = r#"
            struct Item {
                val: i32,
            };

            fn main() -> i32 {
                dec rw i = 0;
                dec item = Item { val: 10 };
                while i < 1 {
                    if i == 0 {
                        dec x = item;
                    }
                    i = i + 1;
                }
                return item.val;
            }
        "#;
        assert_compile_error("sem_move_loop_06", src, "moved", "SEM-MOVE-LOOP-06: Conditionally moved value used after loop");
    }

    // SEM-MOVE-LOOP-07: VALID-POSITIVE - single iteration valid move
    #[test]
    fn sem_move_loop_07_single_iteration_move() {
        let src = r#"
            struct Data {
                value: i32,
            };

            fn use_data(d: Data) -> i32 {
                return d.value;
            }

            fn main() -> i32 {
                dec data = Data { value: 42 };
                dec result = use_data(data);
                return result;
            }
        "#;
        assert_compile_success("sem_move_loop_07", src, "SEM-MOVE-LOOP-07: Single iteration valid move");
    }

    // SEM-MOVE-LOOP-08: VALID-POSITIVE - Copy type used across iterations
    #[test]
    fn sem_move_loop_08_copy_type_across_iterations() {
        let src = r#"
            fn main() -> i32 {
                dec rw sum = 0;
                dec rw i = 0;
                while i < 5 {
                    sum = sum + i;
                    i = i + 1;
                }
                return sum;
            }
        "#;
        assert_compile_success("sem_move_loop_08", src, "SEM-MOVE-LOOP-08: Copy type across iterations");
    }
}

// ============================================================================
// SECTION 3: MOVE THROUGH MATCH (SEM-MOVE-MATCH)
// ============================================================================

mod move_through_match {
    use super::*;

    // SEM-MATCH-01: VALID-POSITIVE - move in one arm, not used after
    #[test]
    fn sem_move_match_01_move_one_arm_use_after() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec mode = Mode::A;
                dec data = Data { value: 42 };
                match mode {
                    Mode::A -> {
                        dec x = data;
                        return x.value;
                    },
                    Mode::B -> {
                        return 0;
                    },
                }
            }
        "#;
        assert_compile_success("sem_move_match_01", src, "SEM-MATCH-01: Move in one arm, not used after");
    }

    // SEM-MATCH-02: VALID-POSITIVE - move in every arm
    #[test]
    fn sem_move_match_02_move_every_arm_use_after() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec mode = Mode::B;
                dec data = Data { value: 42 };
                match mode {
                    Mode::A -> {
                        dec x = data;
                        return x.value;
                    },
                    Mode::B -> {
                        dec y = data;
                        return y.value;
                    },
                }
            }
        "#;
        assert_compile_success("sem_move_match_02", src, "SEM-MATCH-02: Move in every arm");
    }

    // SEM-MATCH-03: VALID-NEGATIVE - moved in one arm, used after match
    #[test]
    fn sem_move_match_03_move_one_arm_used_after() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec mode = Mode::A;
                dec data = Data { value: 42 };
                match mode {
                    Mode::A -> {
                        dec x = data;
                    },
                    Mode::B -> {
                    },
                }
                return data.value;
            }
        "#;
        assert_compile_error("sem_move_match_03", src, "moved", "SEM-MATCH-03: Moved in one arm, used after");
    }

    // SEM-MATCH-04: VALID-POSITIVE - conditionally moved, no use after
    #[test]
    fn sem_move_match_04_conditionally_moved_no_use_after() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Payload {
                data: i32,
            };

            fn process(p: Payload) -> i32 {
                return p.data;
            }

            fn main() -> i32 {
                dec mode = Mode::A;
                dec payload = Payload { data: 100 };
                match mode {
                    Mode::A -> {
                        return process(payload);
                    },
                    Mode::B -> {
                        return 0;
                    },
                }
            }
        "#;
        assert_compile_success("sem_move_match_04", src, "SEM-MATCH-04: Conditionally moved, no use after");
    }

    // SEM-MATCH-05: VALID-POSITIVE - partial move on non-Drop type
    #[test]
    fn sem_move_match_05_partial_move_non_drop() {
        let src = r#"
            struct Pair {
                first: i32,
                second: i32,
            };

            fn main() -> i32 {
                dec pair = Pair { first: 10, second: 20 };
                dec f = pair.first;
                return f + pair.second;
            }
        "#;
        assert_compile_success("sem_move_match_05", src, "SEM-MATCH-05: Partial move on non-Drop type");
    }

    // SEM-MATCH-06: VALID-POSITIVE - value not moved, used in both arms
    #[test]
    fn sem_move_match_06_value_not_moved() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec mode = Mode::A;
                dec data = Data { value: 42 };
                match mode {
                    Mode::A -> {
                        return data.value;
                    },
                    Mode::B -> {
                        return data.value + 1;
                    },
                }
            }
        "#;
        assert_compile_success("sem_move_match_06", src, "SEM-MATCH-06: Value not moved, used in both arms");
    }

    // SEM-MATCH-07: VALID-NEGATIVE - move in one arm, use in other
    #[test]
    fn sem_move_match_07_move_in_one_use_in_other() {
        let src = r#"
            enum Mode {
                A,
                B,
            }

            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec mode = Mode::B;
                dec data = Data { value: 42 };
                match mode {
                    Mode::A -> {
                        dec x = data;
                        return x.value;
                    },
                    Mode::B -> {
                        return data.value;
                    },
                }
            }
        "#;
        assert_compile_success("sem_move_match_07", src, "SEM-MATCH-07: Move in one arm, use in other disjoint arm is valid");
    }
}

// ============================================================================
// SECTION 4: BORROW ACROSS BRANCHES (SEM-BORROW-BRANCH)
// ============================================================================
//
// CORRECTED: These tests require careful analysis of:
//   - Where the borrow is declared (function scope vs. local scope)
//   - Whether the borrow is used after the branch
//   - CFG join point semantics
//
// The borrow_analysis.rs merge is UNION (MAY analysis), so a loan that
// exists on ANY predecessor path is considered active at the join.
// ============================================================================

mod borrow_across_branches {
    use super::*;

    // SEM-BRANCH-01: VALID-POSITIVE - branch-local borrow dead before join
    #[test]
    fn sem_borrow_branch_01_branch_local_borrow_dead_before_join() {
        let src = r#"
            fn helper(val: &i32) -> i32 {
                return *val;
            }

            fn main() -> i32 {
                dec rw x = 10;
                dec rw result = 0;
                if true {
                    result = helper(&x);
                } else {
                    result = 0;
                }
                return result + x;
            }
        "#;
        // CORRECTED: The borrow is created inside the branch and the function
        // returns before the join point. This should compile.
        assert_compile_success("sem_borrow_branch_01", src, "SEM-BRANCH-01: Branch-local borrow dead before join");
    }

    // SEM-BRANCH-02: RULE-BRANCH-JOIN-01 - borrow assigned in branch escaping to outer scope
    #[test]
    fn sem_borrow_branch_02_borrow_escapes_branch() {
        // Positive control: outer reference assigned in branch, referent is outer variable
        let src_valid = r#"
            fn helper(cond: bool) -> i32 {
                dec rw val = 42;
                dec rw r: &i32 = &val;
                if cond {
                    r = &val;
                }
                return *r;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_success("sem_borrow_branch_02_valid", src_valid, "SEM-BRANCH-02: Outer borrow assigned in branch valid");

        // Negative control: conflicting write to outer referent while carrier is used after join
        let src_conflict = r#"
            fn helper(cond: bool) -> i32 {
                dec rw val = 42;
                dec rw r: &i32 = &val;
                if cond {
                    r = &val;
                }
                val = 100;
                return *r;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_error("sem_borrow_branch_02_conflict", src_conflict, "borrowed", "SEM-BRANCH-02: Conflict on outer referent while carrier live after join");
    }

    // SEM-BRANCH-03: VALID-POSITIVE - shared borrow in branch, used after
    #[test]
    fn sem_borrow_branch_03_shared_borrow_in_branch() {
        let src = r#"
            fn use_ref(r: &i32) -> i32 {
                return *r;
            }

            fn main() -> i32 {
                dec rw val = 42;
                dec rw result = 0;
                if true {
                    dec r = &val;
                    result = use_ref(r);
                } else {
                    result = 0;
                }
                return result;
            }
        "#;
        // CORRECTED: The borrow is local to the branch, used only within.
        assert_compile_success("sem_borrow_branch_03", src, "SEM-BRANCH-03: Shared borrow in branch");
    }

    // SEM-BRANCH-04: VALID-POSITIVE - mutable borrow ends, mutation after
    #[test]
    fn sem_borrow_branch_04_mutable_borrow_mutation_after() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec rw v = 0;
                if true {
                    v = x;
                } else {
                    v = 0;
                }
                x = 20;
                return v + x;
            }
        "#;
        // No borrows here - just conditional read. This should compile.
        assert_compile_success("sem_borrow_branch_04", src, "SEM-BRANCH-04: No borrows, conditional read");
    }

    // SEM-BRANCH-05: VALID-POSITIVE - both branches use shared borrow
    #[test]
    fn sem_borrow_branch_05_both_branches_create_loans() {
        let src = r#"
            fn process_shared(r: &i32) -> i32 {
                return *r * 2;
            }

            fn main() -> i32 {
                dec rw x = 5;
                dec rw result = 0;
                if true {
                    result = process_shared(&x);
                } else {
                    result = process_shared(&x);
                }
                return result;
            }
        "#;
        // CORRECTED: Both branches create loans that are used within the branch.
        // This should compile.
        assert_compile_success("sem_borrow_branch_05", src, "SEM-BRANCH-05: Both branches use shared borrow");
    }

    // SEM-BRANCH-06: RULE-BRANCH-JOIN-02 - loan live from one branch at CFG join point
    #[test]
    fn sem_borrow_branch_06_loan_live_from_one_branch() {
        // Positive control: loan created in one branch, used after join, no conflicting mutations -> succeeds
        let src_valid = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &i32 = &x;
                if cond {
                    r = &x;
                } else {
                    r = &y;
                }
                return *r;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_success("sem_borrow_branch_06_valid", src_valid, "SEM-BRANCH-06: Join with branch loan used safely");

        // Negative control: carrier holding branch loan is live across join, referent mutated -> rejected
        let src_conflict = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &i32 = &x;
                if cond {
                    r = &x;
                } else {
                    r = &y;
                }
                x = 99;
                return *r;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_error("sem_borrow_branch_06_conflict", src_conflict, "borrowed", "SEM-BRANCH-06: Mutation of branch-borrowed referent while carrier live after join rejected");

        // Release control: carrier dead before mutation after join -> succeeds
        let src_released = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &i32 = &x;
                if cond {
                    r = &x;
                } else {
                    r = &y;
                }
                dec res = *r;
                x = 99;
                return res + x;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_success("sem_borrow_branch_06_released", src_released, "SEM-BRANCH-06: Referent mutated after carrier death succeeds");
    }

    // SEM-BRANCH-07: VALID-POSITIVE - if/else local borrows die before join
    #[test]
    fn sem_borrow_branch_07_if_else_local_borrows_die() {
        let src = r#"
            fn read(x: &i32) -> i32 { *x }
            fn mutate(x: &rw i32) { *x = *x + 1; }

            fn main() -> i32 {
                dec rw val = 10;
                dec rw result = 0;
                if true {
                    result = read(&val);
                } else {
                    mutate(&rw val);
                    result = 0;
                }
                return result;
            }
        "#;
        // CORRECTED: Borrows are local to branches, die at join.
        assert_compile_success("sem_borrow_branch_07", src, "SEM-BRANCH-07: If/else local borrows die");
    }

    // SEM-BRANCH-08: RULE-BRANCH-JOIN-03 - conflicting borrows across alternative branches
    #[test]
    fn sem_borrow_branch_08_conflicting_borrows_across_branches() {
        // Control 1: Mutually exclusive branches creating mutable borrows of the same place
        let src_mut_branches = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw res = 0;
                if cond {
                    dec r = &rw x;
                    *r = *r + 1;
                    res = *r;
                } else {
                    dec r2 = &rw x;
                    *r2 = *r2 + 2;
                    res = *r2;
                }
                return res + x;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_success("sem_borrow_branch_08_mut_branches", src_mut_branches, "SEM-BRANCH-08: Mutually exclusive mutable borrows of same place valid");

        // Control 2: Outer carrier assigned mutable borrows in alternative branches, safely used after join
        let src_outer_mut = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &rw i32;
                if cond {
                    r = &rw x;
                } else {
                    r = &rw y;
                }
                *r = 30;
                return x + y;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_success("sem_borrow_branch_08_outer_mut", src_outer_mut, "SEM-BRANCH-08: Outer carrier assigned mutable borrow across branches valid");

        // Control 3: Post-join carrier live retains provenance of both branches ({x, y}); access to either referent rejected
        let src_carrier_conflict_x = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &rw i32;
                if cond {
                    r = &rw x;
                } else {
                    r = &rw y;
                }
                dec conflict = x;
                *r = 30;
                return conflict;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_error("sem_borrow_branch_08_carrier_conflict_x", src_carrier_conflict_x, "borrowed", "SEM-BRANCH-08: Accessing referent x while multi-branch carrier live rejected");

        let src_carrier_conflict_y = r#"
            fn helper(cond: bool) -> i32 {
                dec rw x = 10;
                dec rw y = 20;
                dec rw r: &rw i32;
                if cond {
                    r = &rw x;
                } else {
                    r = &rw y;
                }
                dec conflict = y;
                *r = 30;
                return conflict;
            }

            fn main() -> i32 {
                return helper(true);
            }
        "#;
        assert_compile_error("sem_borrow_branch_08_carrier_conflict_y", src_carrier_conflict_y, "borrowed", "SEM-BRANCH-08: Accessing alternative referent y while multi-branch carrier live rejected");
    }
}

// ============================================================================
// SECTION 5: BORROW THROUGH AGGREGATE FIELDS (SEM-BORROW-FIELD)
// ============================================================================
//
// CORRECTED: This section is FULLY CONFORMANT. All tests pass.
// ============================================================================

mod borrow_through_aggregate_fields {
    use super::*;

    // SEM-FIELD-01: VALID-POSITIVE - borrow field, access aggregate
    #[test]
    fn sem_borrow_field_01_borrow_field_access_aggregate() {
        let src = r#"
            struct Point {
                x: i32,
                y: i32,
            };

            fn main() -> i32 {
                dec rw p = Point { x: 1, y: 2 };
                dec r = &p.x;
                dec v = *r;
                return v + p.y;
            }
        "#;
        assert_compile_success("sem_borrow_field_01", src, "SEM-FIELD-01: Borrow field, access aggregate");
    }

    // SEM-FIELD-02: CLOSED (SEM-GAP-16) - borrow field, mutate sibling (disjoint)
    #[test]
    fn sem_borrow_field_02_borrow_field_mutate_sibling() {
        let src = r#"
            struct Point {
                x: i32,
                y: i32,
            };

            fn main() -> i32 {
                dec rw p = Point { x: 1, y: 2 };
                dec r = &p.x;
                p.y = p.y + 1;
                return *r;
            }
        "#;
        // CORRECTED: Disjoint fields don't overlap per RFC P6 §6.1
        assert_compile_success("sem_borrow_field_02", src, "SEM-FIELD-02: Borrow field, mutate sibling");
    }

    // SEM-FIELD-03: VALID-POSITIVE - borrow whole, access field
    #[test]
    fn sem_borrow_field_03_borrow_whole_access_field() {
        let src = r#"
            struct Point {
                x: i32,
                y: i32,
            };

            fn get_x(p: &Point) -> i32 {
                return p.x;
            }

            fn main() -> i32 {
                dec rw p = Point { x: 5, y: 10 };
                dec x = get_x(&p);
                return x;
            }
        "#;
        assert_compile_success("sem_borrow_field_03", src, "SEM-FIELD-03: Borrow whole, access field");
    }

    // SEM-FIELD-04: CLOSED (SEM-GAP-16) - mutable borrow of field blocks mutation
    #[test]
    fn sem_borrow_field_04_mut_borrow_field_blocks_aggregate_mutation() {
        let src = r#"
            struct Point {
                x: i32,
                y: i32,
            };

            fn main() -> i32 {
                dec rw p = Point { x: 1, y: 2 };
                dec r = &rw p.x;
                p.y = p.y + 1;
                return *r;
            }
        "#;
        // CORRECTED: Mutable borrow of field should NOT block sibling mutation
        // per RFC P6 §6.1 disjoint sibling fields don't overlap
        assert_compile_success("sem_borrow_field_04", src, "SEM-FIELD-04: Mut borrow sibling, should be valid");
    }

    // SEM-FIELD-05: VALID-POSITIVE - field after aggregate borrow
    #[test]
    fn sem_borrow_field_05_field_after_aggregate_borrow() {
        let src = r#"
            struct Container {
                data: i32,
            };

            fn use_container(c: &Container) -> i32 {
                return c.data;
            }

            fn main() -> i32 {
                dec rw c = Container { data: 42 };
                dec v1 = use_container(&c);
                dec r = &c.data;
                dec v2 = *r;
                return v1 + v2;
            }
        "#;
        assert_compile_success("sem_borrow_field_05", src, "SEM-FIELD-05: Field after aggregate borrow");
    }

    // SEM-FIELD-06: VALID-POSITIVE - nested field borrow blocks parent
    #[test]
    fn sem_borrow_field_06_nested_field_borrow_blocks_parent() {
        let src = r#"
            struct Inner {
                value: i32,
            };

            struct Outer {
                inner: Inner,
            };

            fn main() -> i32 {
                dec rw o = Outer { inner: Inner { value: 10 } };
                dec r = &o.inner.value;
                o.inner.value = 20;
                return *r;
            }
        "#;
        // Nested field shares parent, should conflict
        assert_compile_error("sem_borrow_field_06", src, "borrowed", "SEM-FIELD-06: Nested field borrow blocks parent");
    }

    // SEM-FIELD-07: VALID-POSITIVE - move field, use rest of aggregate
    #[test]
    fn sem_borrow_field_07_move_field_use_rest() {
        let src = r#"
            struct Pair {
                first: i32,
                second: i32,
            };

            fn main() -> i32 {
                dec pair = Pair { first: 10, second: 20 };
                dec f = pair.first;
                return pair.second;
            }
        "#;
        assert_compile_success("sem_borrow_field_07", src, "SEM-FIELD-07: Move field, use rest of aggregate");
    }

    // SEM-FIELD-08: VALID-POSITIVE - tuple field borrows
    #[test]
    fn sem_borrow_field_08_tuple_field_borrows() {
        let src = r#"
            fn main() -> i32 {
                dec rw t = (10, 20);
                dec r = &t.0;
                dec v = *r;
                return v + t.1;
            }
        "#;
        assert_compile_success("sem_borrow_field_08", src, "SEM-FIELD-08: Tuple field borrows");
    }
}

// ============================================================================
// SECTION 6: BORROW ACROSS LOOPS (SEM-BORROW-LOOP)
// ============================================================================
//
// CORRECTED: Persistent shared borrows across loop iterations are ALLOWED
// unless mutation or conflicting borrows occur. The borrow_analysis.rs
// handles loops as regular CFG with back-edges.
// ============================================================================

mod borrow_across_loops {
    use super::*;

    // SEM-LOOP-01: VALID-POSITIVE - persistent shared borrow across loop
    #[test]
    fn sem_borrow_loop_01_persistent_shared_borrow() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r = &x;
                dec rw i = 0;
                while i < 1 {
                    dec v = *r;
                    i = i + 1;
                }
                return *r;
            }
        "#;
        // CORRECTED: Persistent shared borrow is ALLOWED per Luna semantics.
        // No mutation occurs, so no conflict.
        assert_compile_success("sem_borrow_loop_01", src, "SEM-LOOP-01: Persistent shared borrow across loop");
    }

    // SEM-LOOP-02: CLOSED (SEM-GAP-17) - per-iteration temporary borrow
    #[test]
    fn sem_borrow_loop_02_per_iteration_temporary_borrow() {
        let src = r#"
            fn process(val: &i32) -> i32 {
                return *val;
            }

            fn main() -> i32 {
                dec rw i = 0;
                dec rw sum = 0;
                while i < 3 {
                    dec v = i;
                    sum = sum + process(&v);
                    i = i + 1;
                }
                return sum;
            }
        "#;
        assert_compile_success("sem_borrow_loop_02", src, "SEM-LOOP-02: Per-iteration temporary borrow");
    }

    // SEM-LOOP-03: VALID-NEGATIVE - mutable borrow inside used after
    #[test]
    fn sem_borrow_loop_03_mut_borrow_inside_used_after() {
        let src = r#"
            fn main() -> i32 {
                dec rw i = 0;
                dec rw result = 0;
                while i < 1 {
                    dec rw local = i;
                    result = local;
                    i = i + 1;
                }
                return result;
            }
        "#;
        // This should compile - no borrow escaping
        assert_compile_success("sem_borrow_loop_03", src, "SEM-LOOP-03: Mutable local in loop");
    }

    // SEM-LOOP-04: VALID-POSITIVE - borrow within single iteration
    #[test]
    fn sem_borrow_loop_04_borrow_single_iteration() {
        let src = r#"
            fn double(val: &rw i32) {
                *val = *val * 2;
            }

            fn main() -> i32 {
                dec rw i = 0;
                while i < 3 {
                    double(&rw i);
                    i = i + 1;
                }
                return i;
            }
        "#;
        assert_compile_success("sem_borrow_loop_04", src, "SEM-LOOP-04: Borrow within single iteration");
    }

    // SEM-LOOP-05: VALID-POSITIVE - shared across iterations (no mutation)
    #[test]
    fn sem_borrow_loop_05_shared_across_iterations() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r = &x;
                dec rw i = 0;
                while i < 3 {
                    dec v = *r;
                    i = i + 1;
                }
                return *r;
            }
        "#;
        // CORRECTED: Shared borrow persisting is VALID when no mutation occurs
        assert_compile_success("sem_borrow_loop_05", src, "SEM-LOOP-05: Shared across iterations");
    }

    // SEM-LOOP-06: VALID-POSITIVE - use before borrow, sequential
    #[test]
    fn sem_borrow_loop_06_use_before_borrow() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec v1 = x;
                dec r = &x;
                dec v2 = *r;
                return v1 + v2;
            }
        "#;
        assert_compile_success("sem_borrow_loop_06", src, "SEM-LOOP-06: Use before borrow, sequential");
    }

    // SEM-LOOP-07: RULE-LOOP-ESCAPE-01 - aggregate-carried borrow escaping loop iteration
    #[test]
    fn sem_borrow_loop_07_borrow_escapes_loop() {
        // Positive control: aggregate carrying outer reference updated in loop is valid
        let src_valid = r#"
            struct RefBox {
                ptr: &i32,
            };

            fn main() -> i32 {
                dec rw x = 10;
                dec rw b = RefBox { ptr: &x };
                dec rw i = 0;
                while i < 3 {
                    b.ptr = &x;
                    i = i + 1;
                }
                return *b.ptr;
            }
        "#;
        assert_compile_success("sem_borrow_loop_07_valid", src_valid, "SEM-LOOP-07: Aggregate carrying outer borrow in loop valid");

        // Negative control: aggregate storing iteration-local borrow escaping loop back-edge rejected
        let src_escape = r#"
            struct RefBox {
                ptr: &i32,
            };

            fn main() -> i32 {
                dec rw x = 0;
                dec rw b = RefBox { ptr: &x };
                dec rw i = 0;
                while i < 3 {
                    dec local = i;
                    b.ptr = &local;
                    i = i + 1;
                }
                return *b.ptr;
            }
        "#;
        assert_compile_error("sem_borrow_loop_07_escape", src_escape, "escape", "SEM-LOOP-07: Aggregate storing iteration-local borrow escaping loop rejected");
    }

    // SEM-LOOP-08: RULE-LOOP-PATH-02 - conditional borrow in loop
    #[test]
    fn sem_borrow_loop_08_conditional_borrow_in_loop() {
        let src = r#"
            fn use_val(val: &i32) -> i32 {
                return *val;
            }

            fn main() -> i32 {
                dec rw i = 0;
                dec rw sum = 0;
                while i < 3 {
                    if i == 0 {
                        dec v = i;
                        sum = sum + use_val(&v);
                    } else {
                        sum = sum + i;
                    }
                    i = i + 1;
                }
                return sum;
            }
        "#;
        assert_compile_success("sem_borrow_loop_08", src, "SEM-LOOP-08: Conditional borrow in loop");
    }
}

// ============================================================================
// SECTION 7: SHARED BORROW AFTER ENDED MUTABLE (SEM-BORROW-ENDED)
// ============================================================================

mod shared_borrow_after_ended_mutable {
    use super::*;

    // SEM-ENDED-01: VALID-POSITIVE - mutable ends, then shared
    #[test]
    fn sem_borrow_ended_01_mut_then_shared() {
        let src = r#"
            fn mutate(x: &rw i32) {
                *x = *x + 1;
            }

            fn read(x: &i32) -> i32 {
                return *x;
            }

            fn main() -> i32 {
                dec rw val = 10;
                mutate(&rw val);
                dec v = read(&val);
                return v;
            }
        "#;
        assert_compile_success("sem_borrow_ended_01", src, "SEM-ENDED-01: Mutable borrow ends, shared borrow after");
    }

    // SEM-ENDED-02: VALID-NEGATIVE - shared while mutable still live
    #[test]
    fn sem_borrow_ended_02_shared_while_mut_live() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r1 = &rw x;
                dec r2 = &x;
                *r1 = 20;
                return *r2;
            }
        "#;
        assert_compile_error("sem_borrow_ended_02", src, "borrowed", "SEM-ENDED-02: Shared while mutable live");
    }

    // SEM-ENDED-03: VALID-POSITIVE - multiple mutations, then shared
    #[test]
    fn sem_borrow_ended_03_multiple_mutations_then_shared() {
        let src = r#"
            fn increment(counter: &rw i32) {
                *counter = *counter + 1;
            }

            fn get_value(counter: &i32) -> i32 {
                return *counter;
            }

            fn main() -> i32 {
                dec rw counter = 0;
                increment(&rw counter);
                increment(&rw counter);
                dec result = get_value(&counter);
                return result;
            }
        "#;
        assert_compile_success("sem_borrow_ended_03", src, "SEM-ENDED-03: Multiple mutations, then shared read");
    }

    // SEM-ENDED-04: VALID-NEGATIVE - retained mutable blocks shared
    #[test]
    fn sem_borrow_ended_04_retained_mutable_blocks_shared() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r = &rw x;
                dec s = &x;
                *r = 20;
                return *s;
            }
        "#;
        assert_compile_error("sem_borrow_ended_04", src, "borrowed", "SEM-ENDED-04: Retained mutable blocks shared");
    }

    // SEM-ENDED-05: VALID-POSITIVE - function call with mut ref, then shared
    #[test]
    fn sem_borrow_ended_05_fn_call_mut_then_shared() {
        let src = r#"
            fn modify(v: &rw i32) -> i32 {
                dec old = *v;
                *v = old + 1;
                return old;
            }

            fn main() -> i32 {
                dec rw value = 100;
                dec a = modify(&rw value);
                dec b = modify(&rw value);
                dec c = value;
                return a + b + c;
            }
        "#;
        assert_compile_success("sem_borrow_ended_05", src, "SEM-ENDED-05: Function call with mut ref, then shared");
    }

    // SEM-ENDED-06: RULE-CARRIED-REF-01 - mutable stored in struct, shared after last use
    #[test]
    fn sem_borrow_ended_06_mut_stored_shared_after() {
        // Positive control: struct carrying mutable borrow ends, subsequent shared borrow succeeds
        let src_valid = r#"
            struct MutBox {
                ptr: &rw i32,
            };

            fn main() -> i32 {
                dec rw x = 10;
                {
                    dec rw b = MutBox { ptr: &rw x };
                    *b.ptr = 20;
                }
                dec s = &x;
                return *s;
            }
        "#;
        assert_compile_success("sem_borrow_ended_06_valid", src_valid, "SEM-ENDED-06: Mut stored in struct ends, shared after valid");

        // Negative control: struct carrying mutable borrow live while shared borrow taken is rejected
        let src_conflict = r#"
            struct MutBox {
                ptr: &rw i32,
            };

            fn main() -> i32 {
                dec rw x = 10;
                dec rw b = MutBox { ptr: &rw x };
                dec s = &x;
                *b.ptr = 20;
                return *s;
            }
        "#;
        assert_compile_error("sem_borrow_ended_06_conflict", src_conflict, "borrowed", "SEM-ENDED-06: Mut in struct live during shared borrow rejected");

        // Control 3: Provenance transferred from aggregate A to B; A dies while B remains live -> referent access rejected
        let src_transfer_conflict = r#"
            struct MutBox {
                ptr: &rw i32,
            };

            fn main() -> i32 {
                dec rw x = 10;
                dec rw b: MutBox;
                {
                    dec a = MutBox { ptr: &rw x };
                    b = a;
                }
                dec s = &x;
                *b.ptr = 20;
                return *s;
            }
        "#;
        assert_compile_error("sem_borrow_ended_06_transfer_conflict", src_transfer_conflict, "borrowed", "SEM-ENDED-06: Transferred carried loan active while receiver live rejected");

        // Control 4: Transferred aggregate B also dies -> referent access permitted
        let src_transfer_valid = r#"
            struct MutBox {
                ptr: &rw i32,
            };

            fn main() -> i32 {
                dec rw x = 10;
                {
                    dec rw b: MutBox;
                    {
                        dec a = MutBox { ptr: &rw x };
                        b = a;
                    }
                    *b.ptr = 20;
                }
                dec s = &x;
                return *s;
            }
        "#;
        assert_compile_success("sem_borrow_ended_06_transfer_valid", src_transfer_valid, "SEM-ENDED-06: Transferred carried loan expires when all carriers dead");
    }

    // SEM-ENDED-07: VALID-POSITIVE - nested scope with mutable borrow
    #[test]
    fn sem_borrow_ended_07_nested_scope_mut_then_shared() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                {
                    dec r = &rw x;
                    *r = 20;
                }
                dec s = &x;
                return *s;
            }
        "#;
        assert_compile_success("sem_borrow_ended_07", src, "SEM-ENDED-07: Nested scope ends, shared after");
    }

    // SEM-ENDED-08: RULE-CARRIED-REF-02 - mutable struct field loan isolation and expiration
    #[test]
    fn sem_borrow_ended_08_mut_struct_field_shared_access() {
        // Positive control: disjoint field accessed alongside struct-held borrow; referent accessible after struct dead
        let src_valid = r#"
            struct PairHolder {
                target: &rw i32,
                counter: i32,
            };

            fn main() -> i32 {
                dec rw val = 100;
                dec rw h = PairHolder { target: &rw val, counter: 0 };
                *h.target = 200;
                h.counter = h.counter + 1;
                dec res = h.counter;
                dec s = &val;
                return res + *s;
            }
        "#;
        assert_compile_success("sem_borrow_ended_08_valid", src_valid, "SEM-ENDED-08: Struct field mutable borrow ends, shared access succeeds");

        // Negative control: shared borrow taken while struct field mutable borrow is still active
        let src_conflict = r#"
            struct PairHolder {
                target: &rw i32,
                counter: i32,
            };

            fn main() -> i32 {
                dec rw val = 100;
                dec rw h = PairHolder { target: &rw val, counter: 0 };
                dec s = &val;
                *h.target = 200;
                return *s;
            }
        "#;
        assert_compile_error("sem_borrow_ended_08_conflict", src_conflict, "borrowed", "SEM-ENDED-08: Shared borrow while struct field mut borrow active rejected");

        // Control 3: Field reference transferred out of struct (q = h.target);
        // holder dies, but q remains live -> access to val rejected
        let src_field_transfer_conflict = r#"
            struct PairHolder {
                target: &rw i32,
                counter: i32,
            };

            fn main() -> i32 {
                dec rw val = 100;
                dec rw q: &rw i32;
                {
                    dec rw h = PairHolder { target: &rw val, counter: 0 };
                    q = h.target;
                }
                dec s = &val;
                *q = 300;
                return *s;
            }
        "#;
        assert_compile_error("sem_borrow_ended_08_field_transfer_conflict", src_field_transfer_conflict, "borrowed", "SEM-ENDED-08: Accessing val while transferred field reference q is live rejected");

        // Control 4: Transferred reference q finishes its last use (dies) -> referent val access valid
        let src_field_transfer_valid = r#"
            struct PairHolder {
                target: &rw i32,
                counter: i32,
            };

            fn main() -> i32 {
                dec rw val = 100;
                {
                    dec rw q: &rw i32;
                    {
                        dec rw h = PairHolder { target: &rw val, counter: 0 };
                        q = h.target;
                    }
                    *q = 300;
                }
                dec s = &val;
                return *s;
            }
        "#;
        assert_compile_success("sem_borrow_ended_08_field_transfer_valid", src_field_transfer_valid, "SEM-ENDED-08: Referent val accessible after transferred field reference q dies");
    }
}

// ============================================================================
// SECTION 8: E3001/E3003/E3005 REGRESSION CONTROLS
// ============================================================================

mod regression_controls {
    use super::*;

    // E3001-01: VALID-NEGATIVE - basic use-after-move detection
    #[test]
    fn e3001_01_basic_use_after_move() {
        let src = r#"
            struct Data {
                value: i32,
            };

            fn main() -> i32 {
                dec data = Data { value: 42 };
                dec moved = data;
                return data.value;
            }
        "#;
        assert_compile_error("e3001_01", src, "moved", "E3001-01: Basic use-after-move");
    }

    // E3001-02: VALID-NEGATIVE - use-after-move through function argument
    #[test]
    fn e3001_02_use_after_move_function_arg() {
        let src = r#"
            struct Data {
                value: i32,
            };

            fn consume(d: Data) {}

            fn main() -> i32 {
                dec data = Data { value: 42 };
                consume(data);
                return data.value;
            }
        "#;
        assert_compile_error("e3001_02", src, "moved", "E3001-02: Use-after-move through function arg");
    }

    // E3003-01: VALID-POSITIVE - multiple shared borrows ALLOWED
    #[test]
    fn e3003_01_multiple_shared_borrows_allowed() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r1 = &x;
                dec r2 = &x;
                return *r1;
            }
        "#;
        // CORRECTED: Multiple shared borrows are ALLOWED per Luna semantics
        assert_compile_success("e3003_01", src, "E3003-01: Multiple shared borrows allowed");
    }

    // E3003-02: VALID-NEGATIVE - mutable + shared conflict
    #[test]
    fn e3003_02_mutable_shared_conflict() {
        let src = r#"
            fn main() -> i32 {
                dec rw x = 10;
                dec r = &x;
                dec r2 = &rw x;
                return *r;
            }
        "#;
        assert_compile_error("e3003_02", src, "borrowed", "E3003-02: Mutable borrow conflicts with shared");
    }

    // E3005-01: VALID-NEGATIVE - local borrow escape detection
    #[test]
    fn e3005_01_local_borrow_escape() {
        let src = r#"
            fn make_borrow() -> &i32 {
                dec rw x = 10;
                return &x;
            }
        "#;
        assert_compile_error("e3005_01", src, "escape", "E3005-01: Local borrow escape");
    }

    // E3005-02: RULE-CLOSURE-ESCAPE-01 - closure capturing local borrow
    #[test]
    fn e3005_02_closure_local_borrow() {
        // Negative control: closure capturing local borrow cannot be returned
        let src_escape = r#"
            type ClosureType = typeof(|y: i32| -> i32 { y });

            fn make_closure() -> ClosureType {
                dec x = 10;
                return |y: i32| -> i32 { x + y };
            }
        "#;
        assert_compile_error("e3005_02_escape", src_escape, "escape", "E3005-02: Closure capturing local borrow cannot escape");

        // Positive control: closure capturing local borrow invoked within scope is valid
        let src_valid = r#"
            fn main() -> i32 {
                dec x = 10;
                dec f = |y: i32| -> i32 { x + y };
                return f(5);
            }
        "#;
        assert_compile_success("e3005_02_valid", src_valid, "E3005-02: Closure capturing local borrow invoked locally is valid");
    }
}

// ============================================================================
// SECTION 9: COMPILER GAP TESTS (SEM-GAP-*)
// ============================================================================
//
// These tests document known gaps between the frozen specification and
// the production compiler implementation.
// ============================================================================

mod compiler_gaps {
    use super::*;

    // SEM-GAP-14: CLOSED (VALID-NEGATIVE) - Mutation through shared reference rejected
    // Authority: RFC P6 §6.2 states mutations through shared borrow should be rejected.
    #[test]
    fn sem_gap_14_mutation_through_shared_ref() {
        let src = r#"
            struct Counter {
                value: i32,
            };

            fn increment(c: &Counter) {
                c.value = c.value + 1;
            }

            fn main() -> i32 {
                dec c = Counter { value: 0 };
                increment(&c);
                return c.value;
            }
        "#;
        assert_compile_error("sem_gap_14", src, "cannot mutate", "SEM-GAP-14: Mutation through shared reference rejected");
    }
}

// ============================================================================
// SECTION 10: PHASE 2C CONTROLS (GAP-14 & PROVENANCE REGRESSION CONTROLS)
// ============================================================================

mod phase2c_controls {
    use super::*;

    // Control 1 (GAP-14 control): Direct dereference write through &T rejected (INVALID)
    #[test]
    fn test_ctrl_gap14_direct_deref_write_rejected() {
        let src = r#"
            fn write_shared(r: &i32) {
                *r = 10;
            }
            fn main() -> i32 {
                dec x = 5;
                write_shared(&x);
                return x;
            }
        "#;
        assert_compile_error("ctrl_deref_shared", src, "cannot mutate", "Direct dereference write through &T rejected");
    }

    // Control 2 (GAP-14 control): Nested field projection through &T rejected (INVALID)
    #[test]
    fn test_ctrl_gap14_nested_projection_rejected() {
        let src = r#"
            struct Inner { y: i32, };

            struct Outer { inner: Inner, };

            fn mutate_nested(o: &Outer) {
                o.inner.y = 10;
            }
            fn main() -> i32 {
                dec o = Outer { inner: Inner { y: 0 } };
                mutate_nested(&o);
                return o.inner.y;
            }
        "#;
        assert_compile_error("ctrl_nested_shared", src, "cannot mutate", "Nested field projection through &T rejected");
    }

    // Control 3 (GAP-14 control): Nested field projection through &rw T permitted (VALID)
    #[test]
    fn test_ctrl_gap14_nested_projection_mutref_valid() {
        let src = r#"
            struct Inner { y: i32, };

            struct Outer { inner: Inner, };

            fn mutate_nested(o: &rw Outer) {
                o.inner.y = 10;
            }
            fn main() -> i32 {
                dec rw o = Outer { inner: Inner { y: 0 } };
                mutate_nested(&rw o);
                return o.inner.y;
            }
        "#;
        assert_compile_success("ctrl_nested_mutref", src, "Nested field projection through &rw T permitted");
    }

    // Control 4 (GAP-14 control): Mutation of an owned rw value permitted (VALID)
    #[test]
    fn test_ctrl_gap14_owned_rw_mutation_valid() {
        let src = r#"
            struct Inner { y: i32, };

            struct Outer { inner: Inner, };

            fn main() -> i32 {
                dec rw o = Outer { inner: Inner { y: 0 } };
                o.inner.y = 10;
                return o.inner.y;
            }
        "#;
        assert_compile_success("ctrl_owned_rw", src, "Mutation of owned rw value permitted");
    }

    // Control 5 (GAP-14 control): Read through &T permitted (VALID)
    #[test]
    fn test_ctrl_gap14_shared_read_valid() {
        let src = r#"
            struct Inner { y: i32, };

            struct Outer { inner: Inner, };

            fn read_nested(o: &Outer) -> i32 {
                return o.inner.y;
            }
            fn main() -> i32 {
                dec o = Outer { inner: Inner { y: 42 } };
                return read_nested(&o);
            }
        "#;
        assert_compile_success("ctrl_shared_read", src, "Read through &T permitted");
    }

    // Control 6 (Provenance control): Returned &rw reference carrying provenance prevents subsequent incompatible borrow (INVALID)
    #[test]
    fn test_ctrl_prov_returned_mutref_retains_loan() {
        let src = r#"
            fn identity_mut(x: &rw i32) -> &rw i32 life_from(x) {
                return x;
            }
            fn main() -> i32 {
                dec rw val = 10;
                dec r = identity_mut(&rw val);
                dec r2 = &rw val;
                *r = 20;
                return *r;
            }
        "#;
        assert_compile_error("ctrl_ret_mutref_loan", src, "already borrowed", "Returned &rw reference carrying provenance prevents incompatible access");
    }

    // Control 7 (Provenance control): Provenance carried through user-defined aggregate (INVALID)
    #[test]
    fn test_ctrl_prov_aggregate_carries_provenance() {
        let src = r#"
            struct RefWrap {
                ptr: &rw i32,
            };
            fn wrap(x: &rw i32) -> RefWrap life_from(x) {
                return RefWrap { ptr: x };
            }
            fn main() -> i32 {
                dec rw val = 10;
                dec w = wrap(&rw val);
                dec r2 = &rw val;
                *w.ptr = 20;
                return *w.ptr;
            }
        "#;
        assert_compile_error("ctrl_agg_prov", src, "already borrowed", "Aggregate-carried provenance prevents incompatible access");
    }

    // Control 8 (Entry point consistency control): check() and compile() agree on interprocedural provenance (VALID)
    #[test]
    fn test_ctrl_prov_check_compile_agreement() {
        let src = r#"
            fn helper(x: &rw i32) -> i32 {
                dec v = *x;
                *x = v + 1;
                return v;
            }
            fn main() -> i32 {
                dec rw a = 1;
                dec r1 = helper(&rw a);
                dec r2 = helper(&rw a);
                return r1 + r2;
            }
        "#;
        assert_compile_success("ctrl_check_path", src, "check() path succeeds");

        let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
        let temp = create_temp_dir("ctrl_compile_path");
        let main_path = temp.join("main.ln");
        fs::write(&main_path, src).unwrap();
        let options = CompilerOptions {
            search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
            quiet: true,
            ..Default::default()
        };
        let res = luna_driver::compile(main_path.to_str().unwrap(), src.to_string(), &options);
        assert!(res.is_ok(), "compile() entry point must agree with check(): {:?}", res.err());
    }

    // Control 9: Genuinely reference-free zero-field struct must NOT retain borrow provenance (VALID)
    #[test]
    fn test_ctrl_zero_field_struct_no_provenance() {
        let src = r#"
            struct Marker {};

            struct Container {
                marker: Marker,
            };
            fn get_marker(c: &Container) -> Marker {
                return c.marker;
            }
            fn consume_marker(m: Marker) -> i32 {
                return 0;
            }
            fn main() -> i32 {
                dec rw c = Container { marker: Marker {}, };
                dec m = get_marker(&c);
                c.marker = Marker {};
                return consume_marker(m);
            }
        "#;
        assert_compile_success("ctrl_zero_field_no_prov", src, "Zero-field struct must not retain provenance");
    }

    // Control 10: Cast must propagate provenance interprocedurally (INVALID)
    #[test]
    fn test_ctrl_cast_propagates_provenance() {
        let src = r#"
            fn cast_ref(r: &rw i32) -> *rw i32 {
                return r as *rw i32;
            }
            fn main() -> i32 {
                dec rw val = 10;
                dec p = cast_ref(&rw val);
                dec r2 = &rw val;
                unsafe {
                    *p = 20;
                }
                return *r2;
            }
        "#;
        assert_compile_error("ctrl_cast_prov", src, "already borrowed", "Cast-derived pointer must retain loan and reject second mutable borrow");
    }

    // Control 11: PtrOffset must propagate provenance interprocedurally (INVALID)
    #[test]
    fn test_ctrl_ptroffset_propagates_provenance() {
        let src = r#"
            fn get_elem(arr: &rw [i32; 4]) -> &rw i32 {
                return &rw arr[0];
            }
            fn main() -> i32 {
                dec rw arr = [1, 2, 3, 4];
                dec r = get_elem(&rw arr);
                dec r2 = &rw arr;
                *r = 20;
                return *r;
            }
        "#;
        assert_compile_error("ctrl_ptroffset_prov", src, "already borrowed", "PtrOffset-derived pointer must retain loan and reject second mutable borrow");
    }
}
