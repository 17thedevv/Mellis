use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::{PathBuf};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_def_site_tests")
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

/// A0: No contract + return local reference MUST reject with E3005 LocalBorrowEscape
#[test]
fn test_a0_no_contract_local_return_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a0_no_contract_local");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad() -> &i32 {
    dec x: i32 = 42;
    return &x;
}
"#;
    let path = dir.join("a0.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A0: No contract + return &local MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "A0: Expected E3005 LocalBorrowEscape, got: {:?}",
        errs
    );
}

/// A1: Direct local escape with life_from(a) MUST reject with E3005 LocalBorrowEscape
#[test]
fn test_a1_direct_local_escape_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a1_direct_local");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad(a: &i32) -> &i32 life_from(a) {
    dec x: i32 = 42;
    return &x;
}
"#;
    let path = dir.join("a1.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A1: Direct local escape MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "A1: Expected E3005 LocalBorrowEscape, got: {:?}",
        errs
    );
}

/// A2: Local escape via temporary MUST reject with E3005 LocalBorrowEscape
#[test]
fn test_a2_local_via_temp_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a2_local_temp");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad(a: &i32) -> &i32 life_from(a) {
    dec x: i32 = 42;
    dec t = &x;
    return t;
}
"#;
    let path = dir.join("a2.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A2: Local escape via temporary MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "A2: Expected E3005 LocalBorrowEscape, got: {:?}",
        errs
    );
}

/// A3: Local escape via struct field MUST reject with E3005 LocalBorrowEscape
#[test]
fn test_a3_local_via_field_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a3_local_field");
    let opts = make_opts(&test_sysroot);

    let src = r#"
struct Point {
    x: i32,
    y: i32,
}

fn bad(a: &i32) -> &i32 life_from(a) {
    dec pt = Point { x: 1, y: 2 };
    return &pt.x;
}
"#;
    let path = dir.join("a3.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A3: Local escape via struct field MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "A3: Expected E3005 LocalBorrowEscape, got: {:?}",
        errs
    );
}

/// A4: life_from(a) + return a MUST succeed
#[test]
fn test_a4_life_from_a_return_a_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a4_life_from_a");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn get_first(a: &i32, b: &i32) -> &i32 life_from(a) {
    return a;
}

fn main() {
    dec x = 10;
    dec y = 20;
    dec r = get_first(&x, &y);
}
"#;
    let path = dir.join("a4.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A4: life_from(a) + return a MUST be accepted! Got: {:?}", res.err());
}

/// A5: life_from(a) + return b MUST reject with E2016 LifetimeConstraintViolation
#[test]
fn test_a5_life_from_a_return_b_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a5_life_from_a_ret_b");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad(a: &i32, b: &i32) -> &i32 life_from(a) {
    return b;
}
"#;
    let path = dir.join("a5.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A5: life_from(a) returning b MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "A5: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// A6: life_from(a | b) + return a MUST succeed
#[test]
fn test_a6_life_from_union_return_a_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a6_union_ret_a");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn pick_a(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    return a;
}
"#;
    let path = dir.join("a6.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A6: life_from(a | b) returning a MUST be accepted! Got: {:?}", res.err());
}

/// A7: life_from(a | b) + return b MUST succeed
#[test]
fn test_a7_life_from_union_return_b_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a7_union_ret_b");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn pick_b(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    return b;
}
"#;
    let path = dir.join("a7.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A7: life_from(a | b) returning b MUST be accepted! Got: {:?}", res.err());
}

/// A8: life_from(a | b) + return &local MUST reject with E3005 LocalBorrowEscape
#[test]
fn test_a8_life_from_union_return_local_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a8_union_ret_local");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    dec x: i32 = 42;
    return &x;
}
"#;
    let path = dir.join("a8.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A8: life_from(a | b) returning local MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E3005") || d.message.contains("LocalBorrowEscape")),
        "A8: Expected E3005 LocalBorrowEscape, got: {:?}",
        errs
    );
}

/// A9a: Nested intra-procedural projection (a -> wrapper -> field -> return) MUST succeed
#[test]
fn test_a9a_nested_intraprocedural_projection_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a9a_nested_intra");
    let opts = make_opts(&test_sysroot);

    let src = r#"
struct Wrapper {
    r: &i32,
}

fn wrap_and_project(a: &i32) -> &i32 life_from(a) {
    dec w = Wrapper { r: a };
    return w.r;
}
"#;
    let path = dir.join("a9a.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A9a: Nested intra-procedural projection MUST be accepted! Got: {:?}", res.err());
}

/// A9b: Nested inter-procedural call (a -> helper(a) -> return) MUST succeed
#[test]
fn test_a9b_nested_interprocedural_projection_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a9b_nested_inter");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn helper(x: &i32) -> &i32 life_from(x) {
    return x;
}

fn caller(a: &i32) -> &i32 life_from(a) {
    dec r = helper(a);
    return r;
}
"#;
    let path = dir.join("a9b.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A9b: Nested inter-procedural projection MUST be accepted! Got: {:?}", res.err());
}

/// A10: Multiple return branches where one branch returns undeclared parameter MUST reject with E2016
#[test]
fn test_a10_multibranch_undeclared_branch_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a10_multibranch");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn bad(a: &i32, b: &i32, flag: bool) -> &i32 life_from(a) {
    if flag {
        return a;
    }
    return b;
}
"#;
    let path = dir.join("a10.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "A10: Branch returning b when contract is life_from(a) MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "A10: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// A11: No contract + valid external parameter return MUST preserve existing semantics (succeed)
#[test]
fn test_a11_no_contract_valid_external_return_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("a11_no_contract_valid");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn identity(x: &i32) -> &i32 {
    return x;
}

fn main() {
    dec v = 100;
    dec r = identity(&v);
}
"#;
    let path = dir.join("a11.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "A11: No contract + valid external return MUST succeed! Got: {:?}", res.err());
}
