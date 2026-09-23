//! SEM-MATURITY-01 Phase 2A source conformance corpus.
//!
//! These cases close selected type/generic/trait coverage rows through the
//! public `luna check` boundary. Every fixture uses user-defined types.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("luna-cli must live below the workspace root")
        .to_path_buf()
}

fn fixture(relative: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join("semantic_maturity")
        .join(relative)
}

fn check(relative: &str) -> Output {
    let main = fixture(relative);
    let fixture_dir = main.parent().expect("fixture must have a parent");
    Command::new(env!("CARGO_BIN_EXE_luna"))
        .arg("check")
        .arg(&main)
        .arg("-I")
        .arg(fixture_dir)
        .arg("-I")
        .arg(workspace_root())
        .output()
        .expect("luna check must start")
}

fn rendered(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_accepts(relative: &str) {
    let output = check(relative);
    assert!(output.status.success(), "{}", rendered(&output));
}

fn assert_rejects(relative: &str, expected_fragment: &str) {
    let output = check(relative);
    let text = rendered(&output);
    assert!(
        !output.status.success(),
        "program was silently accepted:\n{text}"
    );
    assert!(
        text.contains(expected_fragment),
        "expected diagnostic fragment {expected_fragment:?}:\n{text}"
    );
}

/// SEM2A-TYPE-01 / SEM-GAP-10
/// Invariant: generic alias arguments substitute recursively through a nested
/// user-defined alias body. Valid; exercises resolver and type substitution.
/// Previous evidence covered only simple/canonical alias shapes.
#[test]
fn sem2a_type_01_nested_generic_alias_is_accepted() {
    assert_accepts("sem2a_type_01_generic_alias_nested.ln");
}

/// SEM2A-TYPE-02
/// Invariant: a generic alias does not erase an incompatible concrete argument.
/// Invalid; expects an explicit type mismatch, never fallback lowering.
#[test]
fn sem2a_type_02_generic_alias_mismatch_is_rejected() {
    assert_rejects("sem2a_type_02_generic_alias_mismatch.ln", "type mismatch");
}

/// SEM2A-GENERIC-01
/// Invariant: nested user-defined generic applications satisfy a parameterized
/// trait bound with declaration-scoped substitutions. Valid; exercises generic
/// substitution, trait selection, method dispatch, mono, and MVIR.
#[test]
fn sem2a_generic_01_nested_trait_bound_is_accepted() {
    assert_accepts("sem2a_generic_01_nested_trait_bound.ln");
}

/// SEM2A-GENERIC-02
/// Invariant: a nested generic argument lacking its required trait impl is
/// rejected at the bound. Invalid; existing positive tests were insufficient
/// to prove the negative bound contract.
#[test]
fn sem2a_generic_02_missing_nested_bound_is_rejected() {
    assert_rejects(
        "sem2a_generic_02_missing_nested_trait_bound.ln",
        "does not implement trait `Mapper`",
    );
}

/// SEM2A-TRAIT-01
/// Invariant: a parameterized user-defined trait dispatches through a generic
/// `&rw self` receiver. Valid; exercises receiver substitution and borrowck.
#[test]
fn sem2a_trait_01_generic_mutable_receiver_is_accepted() {
    assert_accepts("sem2a_trait_01_generic_rw_receiver.ln");
}

/// SEM2A-TRAIT-02 / SEM-GAP-12
/// Invariant: a shared reference cannot satisfy an `&rw self` receiver.
/// Invalid; expects explicit receiver-mutability rejection before lowering.
#[test]
fn sem2a_trait_02_shared_receiver_for_mutable_method_is_rejected() {
    assert_rejects(
        "sem2a_trait_02_shared_to_rw_receiver.ln",
        "Cannot call method requiring mutable receiver",
    );
}

/// SEM2A-ASSOC-01 / SEM-GAP-11
/// Invariant: a generic inherent associated function is callable through its
/// user-defined type and preserves explicit generic arguments. Valid; previous
/// tests only used no-receiver methods as object-safety negatives.
#[test]
fn sem2a_assoc_01_generic_inherent_function_is_accepted() {
    assert_accepts("sem2a_assoc_01_inherent_function.ln");
}

/// SEM2A-ASSOC-02 / SEM-GAP-11
/// Invariant: a receiver-free trait function implemented for a user-defined
/// type is callable through that type. Valid; exercises trait impl identity.
#[test]
fn sem2a_assoc_02_trait_function_is_accepted() {
    assert_accepts("sem2a_assoc_02_trait_function.ln");
}

/// SEM2A-ASSOC-03
/// Invariant: a receiver method is not silently treated as an associated
/// function. Invalid; structured impl lookup must retain receiver identity.
#[test]
fn sem2a_assoc_03_receiver_method_without_receiver_is_rejected() {
    assert_rejects(
        "sem2a_assoc_03_receiver_method_is_not_function.ln",
        "Symbol 'Counter::read' not found",
    );
}

/// SEM2A-METHOD-01
/// Invariant: an inherent method may introduce its own generic parameter in a
/// generic impl and reconstruct nested return types. Valid; exercises method,
/// impl, and nominal substitutions together.
#[test]
fn sem2a_method_01_generic_inherent_method_is_accepted() {
    assert_accepts("sem2a_method_01_generic_inherent.ln");
}

/// SEM2A-METHOD-02 / SEM-GAP-13
/// Invariant: an explicit method type argument constrains the value argument.
/// Invalid; `replace<bool>(7)` must not be re-inferred as `replace<i32>(7)`.
#[test]
fn sem2a_method_02_explicit_generic_mismatch_is_rejected() {
    assert_rejects(
        "sem2a_method_02_generic_argument_mismatch.ln",
        "type mismatch",
    );
}
