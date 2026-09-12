use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_outlives_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn make_opts(test_sysroot: &Sysroot) -> CompilerOptions {
    CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    }
}

/// O1: Valid longer >= shorter call-sites MUST be accepted.
/// Covers both caller parameters with valid contract and ordered local variables.
#[test]
fn test_o1_valid_outlives_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o1_valid_outlives");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller_params(a: &i32, b: &i32) where outlives(a, b) {
    callee(a, b);
}

fn caller_locals() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    callee(&x, &y);
}
"#;
    let path = dir.join("o1.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "O1: Valid outlives call-site MUST be accepted! Got: {:?}", res.err());
}

/// O2a: Inverted order (shorter passed to longer) with locals MUST reject with E2016.
#[test]
fn test_o2a_inverted_locals_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o2a_inverted_locals");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    callee(&y, &x);
}
"#;
    let path = dir.join("o2a.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "O2a: Inverted local lifetimes MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "O2a: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// O2b: Local variable passed to parameter position requiring longer lifetime MUST reject with E2016.
#[test]
fn test_o2b_local_passed_to_param_position_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o2b_local_param");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller(p: &i32) {
    dec local: i32 = 42;
    callee(&local, p);
}
"#;
    let path = dir.join("o2b.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "O2b: Local passed as longer than caller param MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "O2b: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// O3: Equal lifetimes (reflexive relation, e.g. p >= p, &x >= &x) MUST be accepted.
#[test]
fn test_o3_equal_lifetimes_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o3_equal_lifetimes");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller_param(p: &i32) {
    callee(p, p);
}

fn caller_local() {
    dec x: i32 = 10;
    callee(&x, &x);
}
"#;
    let path = dir.join("o3.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "O3: Equal lifetimes MUST be accepted! Got: {:?}", res.err());
}

/// O4: Transitive outlives (a >= b, b >= c implies a >= c) MUST be accepted.
#[test]
fn test_o4_transitive_outlives_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o4_transitive");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller_transitive(a: &i32, b: &i32, c: &i32) where outlives(a, b) where outlives(b, c) {
    callee(a, c);
}
"#;
    let path = dir.join("o4.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "O4: Transitive outlives chain MUST be accepted! Got: {:?}", res.err());
}

/// O5: Transitive contradiction (caller has a >= b, b >= c but passes callee(c, a)) MUST reject with E2016.
#[test]
fn test_o5_contradiction_cycle_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o5_contradiction");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller_bad(a: &i32, b: &i32, c: &i32) where outlives(a, b) where outlives(b, c) {
    callee(c, a);
}
"#;
    let path = dir.join("o5.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "O5: Passing reverse transitive relation MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "O5: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// O6: Independent parameters without contract passed to constrained callee MUST reject with E2016.
#[test]
fn test_o6_independent_parameters_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o6_independent");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn caller_unconstrained(a: &i32, b: &i32) {
    callee(a, b);
}
"#;
    let path = dir.join("o6.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "O6: Independent parameters calling outlives callee MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "O6: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// O7: Source .ms vs .llib parity for where outlives enforcement.
/// Callee's outlives contract must be strictly enforced when callee is loaded from source or .llib.
#[test]
fn test_o7_source_and_llib_outlives_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o7_llib_parity");
    let provider_path = dir.join("order_lib.ln");
    let llib_path = dir.join("order_lib.llib");
    let consumer_bad_path = dir.join("consumer_bad.ln");
    let consumer_good_path = dir.join("consumer_good.ln");

    let provider_src = r#"
module order_lib {
    export fn enforce_order(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
    }
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write order_lib.ln");

    let consumer_bad_src = r#"
import "order_lib";

fn main() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    order_lib::enforce_order(&y, &x);
}
"#;
    fs::write(&consumer_bad_path, consumer_bad_src).expect("Failed to write consumer_bad.ln");

    let consumer_good_src = r#"
import "order_lib";

fn main() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    order_lib::enforce_order(&x, &y);
}
"#;
    fs::write(&consumer_good_path, consumer_good_src).expect("Failed to write consumer_good.ln");

    let opts = CompilerOptions {
        output_path: Some(dir.join("dummy.obj").to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        quiet: true,
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        no_link: true,
        ..Default::default()
    };

    // --- Phase 1: Source Provider Mode (order_lib.llib does NOT exist) ---
    assert!(!llib_path.exists());
    let res_bad_src = compile(
        consumer_bad_path.to_str().unwrap(),
        consumer_bad_src.to_string(),
        &opts,
    );
    assert!(
        res_bad_src.is_err(),
        "Source mode: Inverted argument order MUST fail constraint checking!"
    );
    let errs_src = res_bad_src.err().unwrap();
    assert!(
        errs_src.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Source mode: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs_src
    );

    let res_good_src = compile(
        consumer_good_path.to_str().unwrap(),
        consumer_good_src.to_string(),
        &opts,
    );
    assert!(
        res_good_src.is_ok(),
        "Source mode: Valid argument order MUST pass! Got: {:?}",
        res_good_src.err()
    );

    // --- Phase 2: Compile provider to binary .llib ---
    let prov_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: true,
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        no_link: true,
        ..Default::default()
    };
    let res_prov = compile(
        provider_path.to_str().unwrap(),
        provider_src.to_string(),
        &prov_opts,
    );
    assert!(res_prov.is_ok(), "Failed to build order_lib.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "order_lib.llib was not created!");

    // Remove source file so resolution relies solely on .llib
    fs::remove_file(&provider_path).expect("Failed to remove order_lib.ln");

    // --- Phase 3: Binary .llib Provider Mode ---
    let res_bad_llib = compile(
        consumer_bad_path.to_str().unwrap(),
        consumer_bad_src.to_string(),
        &opts,
    );
    assert!(
        res_bad_llib.is_err(),
        "Binary .llib mode: Inverted argument order MUST fail constraint checking!"
    );
    let errs_llib = res_bad_llib.err().unwrap();
    assert!(
        errs_llib.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Binary .llib mode: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs_llib
    );

    let res_good_llib = compile(
        consumer_good_path.to_str().unwrap(),
        consumer_good_src.to_string(),
        &opts,
    );
    assert!(
        res_good_llib.is_ok(),
        "Binary .llib mode: Valid argument order MUST pass! Got: {:?}",
        res_good_llib.err()
    );
}

/// O8: Combined multi-argument life_from + where outlives contract.
#[test]
fn test_o8_combined_life_from_and_outlives() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("o8_combined");
    let opts = make_opts(&test_sysroot);

    let src_valid = r#"
fn pick_longer(a: &i32, b: &i32) -> &i32 life_from(a) where outlives(a, b) {
    return a;
}

fn caller(x: &i32, y: &i32) -> &i32 life_from(x) where outlives(x, y) {
    dec r = pick_longer(x, y);
    return r;
}
"#;
    let path_valid = dir.join("o8_valid.ln");
    fs::write(&path_valid, src_valid).unwrap();
    let res_valid = check(path_valid.to_str().unwrap(), src_valid.to_string(), &opts);
    assert!(res_valid.is_ok(), "O8: Valid combined contract call MUST pass! Got: {:?}", res_valid.err());

    let src_invalid = r#"
fn pick_longer(a: &i32, b: &i32) -> &i32 life_from(a) where outlives(a, b) {
    return a;
}

fn caller(x: &i32, y: &i32) -> &i32 life_from(x) where outlives(x, y) {
    dec r = pick_longer(y, x);
    return r;
}
"#;
    let path_invalid = dir.join("o8_invalid.ln");
    fs::write(&path_invalid, src_invalid).unwrap();
    let res_invalid = check(path_invalid.to_str().unwrap(), src_invalid.to_string(), &opts);
    assert!(res_invalid.is_err(), "O8: Invalid outlives call with combined contract MUST be rejected!");
    let errs = res_invalid.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "O8: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// Direction Inversion Regression Test:
/// Strictly proves outlives(longer, shorter) means lifetime(longer) >= lifetime(shorter).
/// An outer variable outlives an inner variable.
/// callee(outer, inner) MUST PASS.
/// callee(inner, outer) MUST REJECT with E2016.
#[test]
fn test_direction_inversion_regression() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("direction_inversion_regression");
    let opts = make_opts(&test_sysroot);

    let src_correct_direction = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn test_valid() {
    dec outer: i32 = 10;
    dec inner: i32 = 20;
    callee(&outer, &inner);
}
"#;
    let path_correct = dir.join("correct.ln");
    fs::write(&path_correct, src_correct_direction).unwrap();
    let res_correct = check(path_correct.to_str().unwrap(), src_correct_direction.to_string(), &opts);
    assert!(res_correct.is_ok(), "Direction regression: callee(outer, inner) MUST pass! Got: {:?}", res_correct.err());

    let src_inverted_direction = r#"
fn callee(longer: &i32, shorter: &i32) where outlives(longer, shorter) {
}

fn test_inverted() {
    dec outer: i32 = 10;
    dec inner: i32 = 20;
    callee(&inner, &outer);
}
"#;
    let path_inverted = dir.join("inverted.ln");
    fs::write(&path_inverted, src_inverted_direction).unwrap();
    let res_inverted = check(path_inverted.to_str().unwrap(), src_inverted_direction.to_string(), &opts);
    assert!(res_inverted.is_err(), "Direction regression: callee(inner, outer) MUST be rejected!");
    let errs = res_inverted.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Direction regression: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}
