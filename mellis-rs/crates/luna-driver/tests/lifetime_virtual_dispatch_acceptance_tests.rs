use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_virt_dispatch_acceptance_tests")
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

/// V1: life_from(self) on trait method -> dynamic call preserves provenance.
#[test]
fn test_v1_dyn_call_preserves_provenance() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v1_dyn_call_preserves_provenance");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct Data {
    val: i32,
}

trait Source {
    fn get_data(self: &Self) -> &Data life_from(self);
}

struct MySource {
    data: Data,
}

impl Source for MySource {
    fn get_data(self: &Self) -> &Data life_from(self) {
        return &self.data;
    }
}

fn call_virt(src: &dyn Source) -> &Data life_from(src) {
    return src.get_data();
}

fn main() -> i32 {
    dec s = MySource { data: Data { val: 42 } };
    dec obj: &dyn Source = &s;
    dec r = call_virt(obj);
    return r.val;
}
"#;
    let path = dir.join("v1.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "V1: Dynamic call preserving provenance MUST pass! Got: {:?}", res.err());
}

/// V2: dynamic call + mutation of original owner -> REJECT while returned ref alive; PASS after ref dies.
#[test]
fn test_v2_mutation_rejected_while_alive_passed_after_dead() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v2_mutation_during_borrow");
    let opts = make_opts(&test_sysroot);

    // Part 2a: Reject mutation while returned reference is alive
    let src_bad = r#"
import <core>;

struct Data {
    val: i32,
}

trait Source {
    fn get_data(self: &Self) -> &Data life_from(self);
}

struct MySource {
    data: Data,
}

impl Source for MySource {
    fn get_data(self: &Self) -> &Data life_from(self) {
        return &self.data;
    }
}

fn bad() -> i32 {
    dec rw s = MySource { data: Data { val: 42 } };
    dec obj: &dyn Source = &s;
    dec r = obj.get_data();
    s.data.val = 100;
    return r.val;
}
"#;
    let path_bad = dir.join("v2_bad.ln");
    fs::write(&path_bad, src_bad).unwrap();
    let res_bad = check(path_bad.to_str().unwrap(), src_bad.to_string(), &opts);
    assert!(res_bad.is_err(), "V2a: Mutation of original owner while returned ref is alive MUST be rejected!");

    // Part 2b: Pass mutation after returned reference is dead
    let src_good = r#"
import <core>;

struct Data {
    val: i32,
}

trait Source {
    fn get_data(self: &Self) -> &Data life_from(self);
}

struct MySource {
    data: Data,
}

impl Source for MySource {
    fn get_data(self: &Self) -> &Data life_from(self) {
        return &self.data;
    }
}

fn good() -> i32 {
    dec rw s = MySource { data: Data { val: 42 } };
    dec obj: &dyn Source = &s;
    dec r = obj.get_data();
    dec val = r.val;
    s.data.val = 100;
    return val;
}
"#;
    let path_good = dir.join("v2_good.ln");
    fs::write(&path_good, src_good).unwrap();
    let res_good = check(path_good.to_str().unwrap(), src_good.to_string(), &opts);
    assert!(res_good.is_ok(), "V2b: Mutation of original owner after returned ref dies MUST pass! Got: {:?}", res_good.err());
}

/// V3: dynamic call via .llib -> identical behavior across binary boundary.
#[test]
fn test_v3_dynamic_call_via_llib() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v3_llib_dyn_dispatch");

    let provider_path = dir.join("source_lib.ln");
    let llib_path = dir.join("source_lib.llib");
    let consumer_bad_path = dir.join("consumer_bad.ln");
    let consumer_good_path = dir.join("consumer_good.ln");

    let provider_src = r#"
module source_lib {
    export struct Data {
        val: i32,
    }

    export trait Source {
        fn get_data(self: &Self) -> &Data life_from(self);
    }

    export struct MySource {
        data: Data,
    }

    impl Source for MySource {
        fn get_data(self: &Self) -> &Data life_from(self) {
            return &self.data;
        }
    }
}
"#;

    let consumer_bad_src = r#"
import <core>;
import "source_lib";

fn bad() -> i32 {
    dec rw s = source_lib::MySource { data: source_lib::Data { val: 42 } };
    dec obj: &dyn source_lib::Source = &s;
    dec r = obj.get_data();
    s.data.val = 999;
    return r.val;
}
"#;

    let consumer_good_src = r#"
import <core>;
import "source_lib";

fn good() -> i32 {
    dec rw s = source_lib::MySource { data: source_lib::Data { val: 42 } };
    dec obj: &dyn source_lib::Source = &s;
    dec r = obj.get_data();
    dec val = r.val;
    s.data.val = 999;
    return val;
}
"#;

    fs::write(&provider_path, provider_src).unwrap();
    fs::write(&consumer_bad_path, consumer_bad_src).unwrap();
    fs::write(&consumer_good_path, consumer_good_src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

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
    assert!(res_prov.is_ok(), "Failed to compile source_lib.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "source_lib.llib was not generated!");

    // Remove source file so resolution relies solely on .llib
    fs::remove_file(&provider_path).expect("Failed to remove source_lib.ln");

    // Binary .llib mode: Bad consumer must be rejected due to borrow conflict
    let res_bad = compile(
        consumer_bad_path.to_str().unwrap(),
        consumer_bad_src.to_string(),
        &opts,
    );
    assert!(res_bad.is_err(), "V3: Mutation of owner while ref active via .llib MUST be rejected!");

    // Binary .llib mode: Good consumer must pass
    let res_good = compile(
        consumer_good_path.to_str().unwrap(),
        consumer_good_src.to_string(),
        &opts,
    );
    assert!(res_good.is_ok(), "V3: Valid dispatch via .llib MUST pass! Got: {:?}", res_good.err());
}

/// V4: implementation omits/violates trait lifetime contract -> REJECT at trait conformance (E2016).
#[test]
fn test_v4_impl_omits_trait_lifetime_contract_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v4_impl_omits_contract");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

struct Data {
    val: i32,
}

trait Source {
    fn get_data(self: &Self) -> &Data life_from(self);
}

struct BadSource {
    data: Data,
}

impl Source for BadSource {
    fn get_data(self: &Self) -> &Data {
        return &self.data;
    }
}
"#;
    let path = dir.join("v4.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "V4: Impl omitting trait lifetime contract MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "V4: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// V5: trait method with outlives(a, b) -> CallVirt enforces it (E2016 on violation).
#[test]
fn test_v5_callvirt_enforces_outlives() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v5_callvirt_outlives");
    let opts = make_opts(&test_sysroot);

    // Bad call: shorter does not outlive longer
    let src_bad = r#"
import <core>;

trait Transformer {
    fn transform(self: &Self, longer: &i32, shorter: &i32) -> void where outlives(longer, shorter);
}

struct MyTransformer {}

impl Transformer for MyTransformer {
    fn transform(self: &Self, longer: &i32, shorter: &i32) -> void where outlives(longer, shorter) {
    }
}

fn test_bad(t: &dyn Transformer) {
    dec y: i32 = 2;
    dec x: i32 = 1;
    t.transform(&x, &y);
}
"#;
    let path_bad = dir.join("v5_bad.ln");
    fs::write(&path_bad, src_bad).unwrap();
    let res_bad = check(path_bad.to_str().unwrap(), src_bad.to_string(), &opts);
    assert!(res_bad.is_err(), "V5: CallVirt violating outlives constraint MUST be rejected!");
    let errs = res_bad.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "V5: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );

    // Good call: longer outlives shorter
    let src_good = r#"
import <core>;

trait Transformer {
    fn transform(self: &Self, longer: &i32, shorter: &i32) -> void where outlives(longer, shorter);
}

struct MyTransformer {}

impl Transformer for MyTransformer {
    fn transform(self: &Self, longer: &i32, shorter: &i32) -> void where outlives(longer, shorter) {
    }
}

fn test_good(t: &dyn Transformer, x: &i32) {
    dec y: i32 = 2;
    t.transform(x, &y);
}
"#;
    let path_good = dir.join("v5_good.ln");
    fs::write(&path_good, src_good).unwrap();
    let res_good = check(path_good.to_str().unwrap(), src_good.to_string(), &opts);
    assert!(res_good.is_ok(), "V5: CallVirt satisfying outlives constraint MUST pass! Got: {:?}", res_good.err());
}

/// V6: implementation tries weaker provenance -> REJECT at trait conformance (E2016).
#[test]
fn test_v6_weaker_provenance_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v6_weaker_provenance");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

trait Selector {
    fn pick(self: &Self, a: &i32, b: &i32) -> &i32 life_from(a);
}

struct MySelector {}

impl Selector for MySelector {
    fn pick(self: &Self, a: &i32, b: &i32) -> &i32 life_from(a | b) {
        return b;
    }
}
"#;
    let path = dir.join("v6.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "V6: Implementation declaring weaker provenance than trait MUST be rejected!");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "V6: Expected E2016 LifetimeConstraintViolation, got: {:?}",
        errs
    );
}

/// V7: implementation has stronger/more restrictive contract -> PASS under covariance.
#[test]
fn test_v7_stronger_provenance_accepted() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("v7_stronger_provenance");
    let opts = make_opts(&test_sysroot);

    let src = r#"
import <core>;

trait Selector {
    fn pick(self: &Self, a: &i32, b: &i32) -> &i32 life_from(a | b);
}

struct MySelector {}

impl Selector for MySelector {
    fn pick(self: &Self, a: &i32, b: &i32) -> &i32 life_from(a) {
        return a;
    }
}
"#;
    let path = dir.join("v7.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "V7: Implementation declaring stronger/more restrictive contract MUST pass under covariance! Got: {:?}", res.err());
}
