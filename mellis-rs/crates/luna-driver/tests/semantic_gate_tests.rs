/// Gate Test: "No Semantic Invalid State Reaches MVIR"
///
/// This test suite mechanically verifies the pipeline invariant:
///
///   If semantic analysis emits errors → compilation MUST stop
///   → MVIR generation MUST NOT be invoked
///   → No Operand/artifact is produced from invalid state
///
/// For each semantically invalid program, we verify:
/// 1. `check_semantic_only()` returns Err (semantic gate fires)
/// 2. `check()` returns Err with the SAME diagnostic (full pipeline also stops)
/// 3. The diagnostic contains the expected semantic error code
/// 4. Neither function panics (no ICE from MVIR fallback)
///
/// If a future phase change accidentally bypasses the diagnostic gate,
/// the `check()` call would either:
///   - panic (ICE in MVIR generator on invalid state)
///   - return Ok (silent corruption via Operand::Number("0") fallback)
/// Both are caught by these assertions.

use luna_driver::{check, check_semantic_only, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_semantic_gate_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs both `check_semantic_only()` and `check()` on the given source,
/// asserting that:
///   1. Both return Err (semantic gate fires, MVIR is never reached)
///   2. Both produce at least one diagnostic containing `expected_code`
///   3. Neither panics
fn assert_semantic_gate_fires(test_name: &str, src: &str, expected_code: &str) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();
    let file_str = main_path.to_str().unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // 1. Semantic-only check must produce errors (MVIR is structurally unreachable)
    let sem_result = check_semantic_only(file_str, src.to_string(), &options);
    assert!(
        sem_result.is_err(),
        "[{}] INVARIANT VIOLATION: check_semantic_only() returned Ok for semantically invalid program.\n\
         This means the semantic gate would NOT fire, and MVIR generation could be invoked on invalid state.",
        test_name
    );
    let sem_diags = sem_result.unwrap_err();
    let sem_has_code = sem_diags.iter().any(|d| d.message.contains(expected_code));
    assert!(
        sem_has_code,
        "[{}] Semantic diagnostics do not contain expected code '{}'. Got: {:?}",
        test_name, expected_code, sem_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // 2. Full check() must ALSO produce errors (the same gate fires in the full pipeline)
    let full_result = check(file_str, src.to_string(), &options);
    assert!(
        full_result.is_err(),
        "[{}] INVARIANT VIOLATION: check() returned Ok for semantically invalid program.\n\
         This means MVIR generation was invoked on invalid state and did NOT crash (silent corruption).",
        test_name
    );
    let full_diags = full_result.unwrap_err();
    let full_has_code = full_diags.iter().any(|d| d.message.contains(expected_code));
    assert!(
        full_has_code,
        "[{}] Full pipeline diagnostics do not contain expected code '{}'. Got: {:?}",
        test_name, expected_code, full_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

// =============================================================================
// Gate 1: Unary operator on invalid type
// If this reached MVIR, generator.rs would hit `_ => self.generate_expr(operand)`
// producing a semantically incorrect value with no diagnostic.
// =============================================================================

#[test]
fn test_gate_unary_neg_on_bool() {
    let src = r#"
        fn main() {
            dec x: bool = true;
            dec y = -x;
        }
    "#;
    assert_semantic_gate_fires("gate_unary_neg_on_bool", src, "E_INVALID_UNARY_OP");
}

// =============================================================================
// Gate 2: Range operator (no backend support)
// If this reached MVIR, the generator would panic on BinaryOp::Range
// since there's no Instruction::Range in the MVIR instruction set.
// =============================================================================

#[test]
fn test_gate_range_operator() {
    let src = r#"
        fn main() {
            dec r = 1..10;
        }
    "#;
    assert_semantic_gate_fires("gate_range_operator", src, "E_UNSUPPORTED_FEATURE");
}

// =============================================================================
// Gate 3: Unsized type in value position (ordinary function)
// If this reached MVIR, the generator would produce an Alloca for
// an unsized type, eventually crashing LLVM codegen.
// =============================================================================

#[test]
fn test_gate_unsized_type_in_value_position() {
    let src = r#"
        fn foo(args: [str]) -> i32 {
            return 0;
        }
        fn main() {}
    "#;
    assert_semantic_gate_fires("gate_unsized_value_position", src, "E_UNSIZED_TYPE_IN_VALUE_POSITION");
}

// =============================================================================
// Gate 4: Invalid main signature
// If this reached MVIR, the entrypoint ABI would be silently wrong.
// =============================================================================

#[test]
fn test_gate_invalid_main_signature() {
    let src = r#"
        fn main(x: i32) -> i32 {
            return 0;
        }
    "#;
    assert_semantic_gate_fires("gate_invalid_main_sig", src, "E_INVALID_MAIN_SIGNATURE");
}

// =============================================================================
// Gate 5: Explicit drop() call
// If this reached MVIR, a CallDirect to the drop function would be emitted,
// violating the destructor invariant.
// =============================================================================

#[test]
fn test_gate_explicit_drop_call() {
    let src = r#"
        struct Foo { x: i32 }
        impl core::Drop for Foo {
            fn drop(self: &rw Self) {}
        }
        fn main() {
            dec rw f = Foo { x: 1 };
            f.drop();
        }
    "#;
    assert_semantic_gate_fires("gate_explicit_drop", src, "drop");
}

// =============================================================================
// Gate 6: Unsafe dereference outside unsafe block
// If this reached MVIR, a Load from a raw pointer would be emitted
// without safety verification.
// =============================================================================

#[test]
fn test_gate_unsafe_deref_outside_unsafe() {
    let src = r#"
        fn main() {
            dec p: *i32 = 0 as *i32;
            dec x = *p;
        }
    "#;
    assert_semantic_gate_fires("gate_unsafe_deref", src, "E_UNSAFE_DEREF_OUTSIDE_UNSAFE");
}

// =============================================================================
// Gate 7: Call to unsafe function outside unsafe block
// If this reached MVIR, the call would be emitted without the safety barrier.
// =============================================================================

#[test]
fn test_gate_unsafe_call_outside_unsafe() {
    let src = r#"
        unsafe fn danger() -> i32 { return 42; }
        fn main() {
            dec x = danger();
        }
    "#;
    assert_semantic_gate_fires("gate_unsafe_call", src, "E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE");
}

// =============================================================================
// Gate 8: Unresolved symbol
// If this reached MVIR, a CallDirect/Load would be emitted for a symbol
// that doesn't exist, causing an MVIR/LLVM crash.
// =============================================================================

#[test]
fn test_gate_unresolved_symbol() {
    let src = r#"
        fn main() {
            dec x = nonexistent_function();
        }
    "#;
    assert_semantic_gate_fires("gate_unresolved_symbol", src, "not found");
}

// =============================================================================
// Gate 9: Type mismatch (basic unification failure)
// If this reached MVIR, a Store of wrong type would be emitted,
// eventually crashing LLVM type verification.
// =============================================================================

#[test]
fn test_gate_type_mismatch() {
    let src = r#"
        fn main() {
            dec x: i32 = true;
        }
    "#;
    assert_semantic_gate_fires("gate_type_mismatch", src, "type mismatch");
}

// =============================================================================
// Gate 10: Async main (forbidden entrypoint)
// If this reached MVIR, async lowering would be applied to main,
// producing a corrupted state machine entrypoint.
// =============================================================================

#[test]
fn test_gate_async_main() {
    let src = r#"
        async fn main() {}
    "#;
    assert_semantic_gate_fires("gate_async_main", src, "E_INVALID_MAIN_SIGNATURE");
}

// =============================================================================
// Gate 11: Semantic-valid program MUST reach MVIR (positive invariant)
// This verifies the gate does NOT over-reject valid programs.
// =============================================================================

#[test]
fn test_gate_positive_valid_program_passes() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("gate_positive_valid");
    let main_path = temp.join("main.ln");
    let src = r#"
        fn main() -> i32 {
            dec x: i32 = 42;
            return x;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Semantic-only check passes
    let sem_result = check_semantic_only(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        sem_result.is_ok(),
        "Positive invariant violated: valid program rejected by semantic gate. Diags: {:?}",
        sem_result.unwrap_err()
    );

    // Full check passes (MVIR generation is invoked and succeeds)
    let full_result = check(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(
        full_result.is_ok(),
        "Positive invariant violated: valid program rejected by full pipeline. Diags: {:?}",
        full_result.unwrap_err()
    );
}

// =============================================================================
// Gate 12: Cannot Dereference Non-Pointer/Non-Reference (Regression Bug 1)
// =============================================================================

#[test]
fn test_gate_cannot_deref_integer() {
    let src = r#"
        fn main() {
            dec x = 42;
            dec y = *x;
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_deref_integer", src, "E_CANNOT_DEREFERENCE");
}

#[test]
fn test_gate_cannot_deref_boolean() {
    let src = r#"
        fn main() {
            dec x = true;
            dec y = *x;
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_deref_boolean", src, "E_CANNOT_DEREFERENCE");
}

// =============================================================================
// Gate 13: Cannot Index Non-Array/Non-Slice (Regression Bug 2)
// =============================================================================

#[test]
fn test_gate_cannot_index_integer() {
    let src = r#"
        fn main() {
            dec x = 42;
            dec y: i32 = x[0];
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_index_integer", src, "E_CANNOT_INDEX");
}

#[test]
fn test_gate_cannot_index_boolean() {
    let src = r#"
        fn main() {
            dec x = true;
            dec y = x[0];
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_index_boolean", src, "E_CANNOT_INDEX");
}

// =============================================================================
// Gate 14: Invalid Index Type (Regression Bug 3)
// =============================================================================

#[test]
fn test_gate_cannot_index_with_string() {
    let src = r#"
        fn main() {
            dec arr: [i32; 2] = [1, 2];
            dec y = arr["hello"];
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_index_with_string", src, "E_INVALID_INDEX_TYPE");
}

#[test]
fn test_gate_cannot_index_with_bool() {
    let src = r#"
        fn main() {
            dec arr: [i32; 2] = [1, 2];
            dec y = arr[true];
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_index_with_bool", src, "E_INVALID_INDEX_TYPE");
}

// =============================================================================
// Gate 15: Invalid Lvalue Assignment (Regression Bug 4)
// =============================================================================

#[test]
fn test_gate_cannot_assign_to_literal() {
    let src = r#"
        fn main() {
            42 = 10;
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_assign_to_literal", src, "E_INVALID_LVALUE");
}

#[test]
fn test_gate_cannot_assign_to_binary_expr() {
    let src = r#"
        fn main() {
            dec rw x = 10;
            (x + 1) = 20;
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_assign_to_binary_expr", src, "E_INVALID_LVALUE");
}

#[test]
fn test_gate_cannot_assign_to_call_result() {
    let src = r#"
        fn get_val() -> i32 { return 42; }
        fn main() {
            get_val() = 10;
        }
    "#;
    assert_semantic_gate_fires("gate_cannot_assign_to_call_result", src, "E_INVALID_LVALUE");
}

// =============================================================================
// Comprehensive Adversarial Negative Corpus (22 distinct invalid programs)
// Verifying absence of behavior: check() == Err, zero MVIR, zero panic!
// =============================================================================

#[test]
fn test_adversarial_negative_corpus() {
    let corpus: Vec<(&str, &str, &str)> = vec![
        ("deref_int", "fn main() { dec x = 1; dec y = *x; }", "E_CANNOT_DEREFERENCE"),
        ("deref_bool", "fn main() { dec x = false; dec y = *x; }", "E_CANNOT_DEREFERENCE"),
        ("index_int", "fn main() { dec x = 10; dec y = x[0]; }", "E_CANNOT_INDEX"),
        ("index_str_key", "fn main() { dec arr: [i32; 1] = [1]; dec y = arr[\"key\"]; }", "E_INVALID_INDEX_TYPE"),
        ("index_bool_key", "fn main() { dec arr: [i32; 1] = [1]; dec y = arr[false]; }", "E_INVALID_INDEX_TYPE"),
        ("assign_literal", "fn main() { 99 = 1; }", "E_INVALID_LVALUE"),
        ("assign_binary", "fn main() { dec rw x = 1; (x * 2) = 4; }", "E_INVALID_LVALUE"),
        ("neg_bool", "fn main() { dec b = true; dec c = -b; }", "E_INVALID_UNARY_OP"),
        ("unsized_let", "fn main() { dec x: [i32]; }", "E_UNSIZED_TYPE_IN_VALUE_POSITION"),
        ("unsized_fn_param", "fn foo(x: [i32]) {} fn main() {}", "E_UNSIZED_TYPE_IN_VALUE_POSITION"),
        ("unsafe_deref_safe", "fn main() { dec x = 1; dec p: *i32 = &x as *i32; dec y = *p; }", "E_UNSAFE_DEREF_OUTSIDE_UNSAFE"),
        ("unsafe_arith_safe", "fn main() { dec x = 1; dec p: *i32 = &x as *i32; dec q = p + 1; }", "E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE"),
        ("mut_immutable_ptr", "fn main() { dec x = 1; dec p: *i32 = &x as *i32; unsafe { *p = 2; } }", "E_CANNOT_MUTATE_IMMUTABLE_POINTER"),
        ("mut_immutable_var", "fn main() { dec x = 1; x = 2; }", "Cannot mutate immutable variable"),
        ("range_operator", "fn main() { dec r = 0..10; }", "E_UNSUPPORTED_FEATURE"),
        ("explicit_drop", "struct S { x: i32 } impl Drop for S { fn drop(self: &rw Self) {} } fn main() { dec s = S { x: 1 }; s.drop(); }", "Explicit calls to drop() are forbidden"),
        ("async_main", "async fn main() {}", "E_INVALID_MAIN_SIGNATURE"),
        ("main_args_void", "fn main(args: [str]) -> void {}", "E_INVALID_MAIN_SIGNATURE"),
        ("comptime_div_zero", "const X: i32 = comptime { 10 / 0 }; fn main() {}", "attempt to divide by zero"),
        ("type_mismatch_var", "fn main() { dec x: i32 = true; }", "type mismatch"),
        ("unresolved_sym", "fn main() { missing_fn_call(); }", "not found"),
        ("copy_drop_conflict", "import <core>; struct S { x: i32 } impl Copy for S {} impl Drop for S { fn drop(self: &rw Self) {} } fn main() {}", "E_COPY_DROP_CONFLICT"),
    ];

    for (name, src, expected_err) in corpus {
        assert_semantic_gate_fires(&format!("corpus_{}", name), src, expected_err);
    }
}

