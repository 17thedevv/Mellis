//! SEM-MATURITY-01 Phase 0 source conformance corpus.
//!
//! Every case enters through the public `luna check` command and compiles a
//! standalone `.ln` fixture. Cases that originally reproduced compiler gaps
//! remain active as closure regressions once the frozen behavior is restored.

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

/// SEM-GENERIC-01
/// Invariant: unrelated declarations may reuse generic names, and nested
/// user-defined generic applications preserve their concrete types.
/// Valid; expects acceptance through parser, resolver, type checker, mono, MVIR,
/// and borrowck. Existing coverage was predominantly inline or stdlib-shaped.
#[test]
fn sem_generic_01_nested_user_types_are_accepted() {
    assert_accepts("sem_generic_01_nested_user_types.ln");
}

/// SEM-TRAIT-01
/// Invariant: a user-defined generic trait argument participates in a generic
/// function bound and dispatches to a user-defined impl.
/// Valid; expects acceptance through trait selection and monomorphization.
/// Existing broad suites mixed this rule with stdlib traits.
#[test]
fn sem_trait_01_generic_trait_argument_is_accepted() {
    assert_accepts("sem_trait_01_generic_trait_argument.ln");
}

/// SEM-BORROW-01
/// Invariant: multiple shared field borrows may end before a later mutable field
/// borrow of the same user-defined aggregate.
/// Valid; expects acceptance by NLL/borrowck. Existing tests did not provide an
/// isolated user-defined generic aggregate proof at the public boundary.
#[test]
fn sem_borrow_01_shared_then_mutable_is_accepted() {
    assert_accepts("sem_borrow_01_shared_then_mutable.ln");
}

/// SEM-BORROW-02
/// Invariant: an overlapping shared and mutable field borrow is rejected.
/// Invalid; expects an existing borrow-conflict diagnostic from borrowck.
#[test]
fn sem_borrow_02_conflicting_field_borrow_is_rejected() {
    assert_rejects("sem_borrow_02_conflicting_field_borrow.ln", "Cannot borrow");
}

/// SEM-LIFETIME-01
/// Invariant: `life_from(a | b)` permits either declared input provenance.
/// Valid; expects acceptance by lifetime checking and downstream lowering.
#[test]
fn sem_lifetime_01_union_provenance_is_accepted() {
    assert_accepts("sem_lifetime_01_union_provenance.ln");
}

/// SEM-LIFETIME-02
/// Invariant: a local reference cannot escape under an input `life_from` contract.
/// Invalid; expects local-borrow-escape rejection rather than backend failure.
#[test]
fn sem_lifetime_02_local_escape_is_rejected() {
    assert_rejects("sem_lifetime_02_local_escape.ln", "E3005");
}

/// SEM-OWN-01
/// Invariant: consuming a non-Copy user-defined value makes subsequent use invalid.
/// Invalid; expects use-after-move rejection from borrowck.
#[test]
fn sem_own_01_use_after_move_is_rejected() {
    assert_rejects("sem_own_01_use_after_move.ln", "moved");
}

/// SEM-DROP-01
/// Invariant: moving a field out of a value implementing Drop is forbidden.
/// Invalid; expects partial-move-under-Drop rejection.
#[test]
fn sem_drop_01_partial_move_under_drop_is_rejected() {
    assert_rejects(
        "sem_drop_01_partial_move_under_drop.ln",
        "E3002",
    );
}

/// SEM-ASSOC-01
/// Invariant: an associated type binding and generic projection resolve when the
/// trait, impl, and generic consumer are in one source provider.
/// Valid; this is the control for the cross-provider reproduction.
#[test]
fn sem_assoc_01_same_provider_projection_is_accepted() {
    assert_accepts("sem_assoc_01_same_provider_projection.ln");
}

/// SEM-PROVIDER-01
/// Invariant: two imported providers contributing the same root definition are
/// rejected deterministically as a duplicate definition.
/// Invalid; existing tests cover the behavior, while this fixture keeps the
/// Phase 0 source corpus independently runnable.
#[test]
fn sem_provider_01_duplicate_root_definition_is_rejected() {
    assert_rejects("sem_provider_01_duplicate/main.ln", "E1002");
}

/// SEM-CONST-01
/// Invariant: a pure comptime block may mutate local state and yield a const value.
/// Valid; expects acceptance through comptime evaluation and MVIR generation.
#[test]
fn sem_const_01_local_mutation_is_accepted() {
    assert_accepts("sem_const_01_local_mutation.ln");
}

/// SEM-CONST-02
/// Invariant: comptime division by zero is rejected without a fallback value.
/// Invalid; expects deterministic evaluation failure before backend execution.
#[test]
fn sem_const_02_division_by_zero_is_rejected() {
    assert_rejects("sem_const_02_division_by_zero.ln", "divide by zero");
}

/// SEM-ITER-01 / SEM-GAP-01
/// Invariant: `for-in` lowers through user-defined IntoIterator/Iterator impls.
/// Valid; expected acceptance by trait selection and MVIR lowering. Existing
/// stdlib container tests cannot prove this generic language rule.
#[test]
fn sem_iter_01_user_defined_for_in_is_accepted() {
    assert_accepts("sem_iter_01_user_defined_for_in.ln");
    assert_accepts("sem_iter_02_vec_owned_for_in.ln");
}

/// SEM-METHOD-01 / SEM-GAP-02
/// Invariant: sequential `&rw self` method calls end each receiver borrow at the
/// call boundary when the reference does not escape.
/// Valid; expected acceptance by borrowck on a user-defined receiver.
#[test]
fn sem_method_01_sequential_mut_receivers_are_accepted() {
    assert_accepts("sem_method_01_sequential_mut_receivers.ln");
    assert_rejects(
        "sem_method_02_escaping_mut_receiver_borrow.ln",
        "Cannot borrow",
    );
    assert_rejects(
        "sem_method_03_shared_then_mut_receiver_overlap.ln",
        "Cannot borrow",
    );
}

/// SEM-DIAG-01 / SEM-GAP-03 Phase 1A regression
/// Invariant: calling an undeclared method on a user-defined type is rejected.
/// Invalid; expects an existing unresolved-method/type diagnostic and no lowering.
#[test]
fn sem_diag_01_unknown_method_is_rejected() {
    assert_rejects("sem_diag_01_unknown_method.ln", "not found for type");
}

/// SEM-DIAG-02 / SEM-GAP-04
/// Invariant: use after move is reported using the frozen E3001 code.
/// Invalid; semantic rejection is separately proven by SEM-OWN-01, while this
/// assertion isolates the missing registry identity.
#[test]
fn sem_diag_02_use_after_move_uses_frozen_numeric_code() {
    assert_rejects("sem_own_01_use_after_move.ln", "E3001");
}

/// SEM-DIAG-03 / SEM-GAP-07
/// Invariant: partial move under Drop is reported using the frozen E3002 code,
/// not an internal symbolic label. Invalid; semantic rejection is separately
/// proven by SEM-DROP-01.
#[test]
fn sem_diag_03_partial_move_uses_frozen_numeric_code() {
    assert_rejects("sem_drop_01_partial_move_under_drop.ln", "E3002");
}

/// SEM-DIAG-04 / SEM-GAP-08
/// Invariant: a borrow conflict is reported using the frozen E3003 code.
/// Invalid; semantic rejection is separately proven by SEM-BORROW-02, while
/// this assertion isolates the missing registry identity.
#[test]
fn sem_diag_04_borrow_conflict_uses_frozen_numeric_code() {
    assert_rejects("sem_borrow_02_conflicting_field_borrow.ln", "E3003");
    assert_rejects("sem_borrow_03_mutable_mutable_conflict.ln", "E3003");
}

/// SEM-VIS-01
/// Invariant: fields are public by default, while an explicitly `private`
/// field cannot be read from an importing provider. Invalid; semantic rejection
/// must occur after the containing exported type is resolved.
#[test]
fn sem_vis_01_explicit_private_field_is_rejected_externally() {
    assert_rejects(
        "sem_vis_01_explicit_private_field/main.ln",
        "Field `value` of struct `Secret` is private",
    );
}

/// SEM-DIAG-05 / SEM-GAP-09
/// Invariant: illegal external access to an explicitly private field is
/// reported using the frozen E1003 code. Invalid; semantic rejection is
/// separately proven by SEM-VIS-01.
#[test]
fn sem_diag_05_private_access_uses_frozen_numeric_code() {
    assert_rejects("sem_vis_01_explicit_private_field/main.ln", "E1003");
}

/// SEM-COHERENCE-01 / SEM-GAP-06 Phase 1A regression
/// Invariant: a local type nested beneath a non-local fundamental/reference head
/// does not satisfy the frozen nominal-head orphan rule for a foreign trait.
/// Invalid; expects coherence rejection before monomorphization.
#[test]
fn sem_coherence_01_reference_head_orphan_is_rejected() {
    assert_rejects(
        "sem_coherence_01_reference_head_orphan.ln",
        "E_ORPHAN_IMPL",
    );
}

/// SEM-ASSOC-02
/// Invariant: provider B may publicly expose a projection of an associated type
/// owned by provider A, and consumer C may instantiate and consume B's API.
/// Valid; source-only A -> B -> C coverage exercises provider injection, public
/// signature reconstruction, trait selection, and projection normalization.
#[test]
fn sem_assoc_02_cross_provider_projection_is_accepted() {
    assert_accepts("sem_assoc_02_cross_provider/main.ln");
}
