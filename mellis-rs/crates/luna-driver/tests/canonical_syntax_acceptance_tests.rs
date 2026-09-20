use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_canonical_syntax_tests")
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

/// SYNTAX-01: requires life(a) >= life(b) parsed and enforced on caller arguments.
#[test]
fn test_syntax_01_requires_gte() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_01");
    let opts = make_opts(&test_sysroot);

    let src_valid = r#"
fn callee(longer: &i32, shorter: &i32) requires life(longer) >= life(shorter) {}

fn caller_good(a: &i32, b: &i32) requires life(a) >= life(b) {
    callee(a, b);
}
"#;
    let path_valid = dir.join("valid.ln");
    fs::write(&path_valid, src_valid).unwrap();
    let res = check(path_valid.to_str().unwrap(), src_valid.to_string(), &opts);
    assert!(res.is_ok(), "SYNTAX-01: Valid >= contract must pass: {:?}", res.err());

    let src_invalid = r#"
fn callee(longer: &i32, shorter: &i32) requires life(longer) >= life(shorter) {}

fn caller_bad(a: &i32, b: &i32) requires life(a) >= life(b) {
    callee(b, a);
}
"#;
    let path_invalid = dir.join("invalid.ln");
    fs::write(&path_invalid, src_invalid).unwrap();
    let res = check(path_invalid.to_str().unwrap(), src_invalid.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-01: Inverted arguments must fail");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// SYNTAX-02: requires life(b) <= life(a) normalized to same longer=a, shorter=b.
#[test]
fn test_syntax_02_requires_lte() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_02");
    let opts = make_opts(&test_sysroot);

    let src_valid = r#"
fn callee(longer: &i32, shorter: &i32) requires life(shorter) <= life(longer) {}

fn caller_good(a: &i32, b: &i32) requires life(b) <= life(a) {
    callee(a, b);
}
"#;
    let path_valid = dir.join("valid.ln");
    fs::write(&path_valid, src_valid).unwrap();
    let res = check(path_valid.to_str().unwrap(), src_valid.to_string(), &opts);
    assert!(res.is_ok(), "SYNTAX-02: Valid <= contract must pass: {:?}", res.err());

    let src_invalid = r#"
fn callee(longer: &i32, shorter: &i32) requires life(shorter) <= life(longer) {}

fn caller_bad(a: &i32, b: &i32) requires life(b) <= life(a) {
    callee(b, a);
}
"#;
    let path_invalid = dir.join("invalid.ln");
    fs::write(&path_invalid, src_invalid).unwrap();
    let res = check(path_invalid.to_str().unwrap(), src_invalid.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-02: Inverted arguments must fail");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// SYNTAX-03: requires life(self) >= life(other) parsed and resolved on method receiver.
#[test]
fn test_syntax_03_receiver_self() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_03");
    let opts = make_opts(&test_sysroot);

    let src = r#"
struct Container {};

impl Container {
    fn store(self: &Container, other: &i32) requires life(self) >= life(other) {}
}

fn test_valid(c: &Container, x: &i32) requires life(c) >= life(x) {
    c.store(x);
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "SYNTAX-03: Method receiver self constraint must pass: {:?}", res.err());
}

/// SYNTAX-04: requires life(holder.field) >= life(b) parsed as projection; cleanly rejected with UnsupportedContractRelation.
#[test]
fn test_syntax_04_projection_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_04");
    let opts = make_opts(&test_sysroot);

    let src = r#"
struct Holder { val: &i32 };

fn test_proj(h: &Holder, b: &i32) requires life(h.val) >= life(b) {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-04: Projection in requires must be rejected in v1");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("unsupported lifetime contract relation") && d.message.contains("lifetime projection 'h.val'")),
        "Expected unsupported lifetime projection error, got: {:?}", errs
    );
}

/// SYNTAX-05: requires life(return) <= life(a) rejected with UnsupportedContractRelation.
#[test]
fn test_syntax_05_return_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_05");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn get_ref(a: &i32) -> &i32 requires life(return) <= life(a) {
    return a;
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-05: Return in requires must be rejected with informative error");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("generic Return lifetime relations in 'requires' are not supported in Lifetime Contract v1; if the intended contract describes return provenance, use 'life_from(...)'")),
        "Expected UnsupportedContractRelation for return, got: {:?}", errs
    );
}

/// SYNTAX-06: Order independence: life_from(a) requires ... and requires ... life_from(a) behave identically.
#[test]
fn test_syntax_06_order_independence() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_06");
    let opts = make_opts(&test_sysroot);

    let src1 = r#"
fn fn_order_1(a: &i32, b: &i32) -> &i32 life_from(a) requires life(a) >= life(b) {
    return a;
}
"#;
    let path1 = dir.join("order1.ln");
    fs::write(&path1, src1).unwrap();
    let res1 = check(path1.to_str().unwrap(), src1.to_string(), &opts);
    assert!(res1.is_ok(), "SYNTAX-06: life_from before requires must pass: {:?}", res1.err());

    let src2 = r#"
fn fn_order_2(a: &i32, b: &i32) -> &i32 requires life(a) >= life(b) life_from(a) {
    return a;
}
"#;
    let path2 = dir.join("order2.ln");
    fs::write(&path2, src2).unwrap();
    let res2 = check(path2.to_str().unwrap(), src2.to_string(), &opts);
    assert!(res2.is_ok(), "SYNTAX-06: requires before life_from must pass: {:?}", res2.err());
}

/// SYNTAX-07: Multiple requires clauses (chained or comma-separated) enforce transitively.
#[test]
fn test_syntax_07_multiple_requires() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_07");
    let opts = make_opts(&test_sysroot);

    // Chained: requires ... requires ...
    let src_chained = r#"
fn callee(a: &i32, c: &i32) requires life(a) >= life(c) {}

fn caller_chained(a: &i32, b: &i32, c: &i32) requires life(a) >= life(b) requires life(b) >= life(c) {
    callee(a, c);
}
"#;
    let path_chained = dir.join("chained.ln");
    fs::write(&path_chained, src_chained).unwrap();
    let res_c = check(path_chained.to_str().unwrap(), src_chained.to_string(), &opts);
    assert!(res_c.is_ok(), "SYNTAX-07: Chained requires must pass: {:?}", res_c.err());

    // Comma-separated: requires ..., ...
    let src_comma = r#"
fn callee(a: &i32, c: &i32) requires life(a) >= life(c) {}

fn caller_comma(a: &i32, b: &i32, c: &i32) requires life(a) >= life(b), life(b) >= life(c) {
    callee(a, c);
}
"#;
    let path_comma = dir.join("comma.ln");
    fs::write(&path_comma, src_comma).unwrap();
    let res_comma = check(path_comma.to_str().unwrap(), src_comma.to_string(), &opts);
    assert!(res_comma.is_ok(), "SYNTAX-07: Comma-separated requires must pass: {:?}", res_comma.err());
}

/// SYNTAX-08: Legacy migration diagnostic: where outlives(a, b) produces targeted migration error.
#[test]
fn test_syntax_08_legacy_migration_diagnostic() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_08");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn legacy_fn(a: &i32, b: &i32) where outlives(a, b) {}
"#;
    let path = dir.join("legacy.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-08: Legacy where outlives must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("'where outlives(...)' has been removed; use canonical 'requires life(a) >= life(b)'")),
        "Expected targeted migration error, got: {:?}", errs
    );
}

/// SYNTAX-09: Unknown parameter in requires reports lifetime does not refer to any parameter in scope.
#[test]
fn test_syntax_09_unknown_param() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_09");
    let opts = make_opts(&test_sysroot);

    let src = r#"
fn unknown_param_fn(a: &i32) requires life(nonexistent) >= life(a) {}
"#;
    let path = dir.join("unknown.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "SYNTAX-09: Unknown parameter must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("lifetime 'nonexistent' does not refer to any parameter in scope")),
        "Expected unresolved lifetime error, got: {:?}", errs
    );
}

/// SYNTAX-10: .llib / ABI serialization and deserialization preserves canonical contract from requires.
#[test]
fn test_syntax_10_llib_contract_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_10");
    let provider_path = dir.join("my_lib.ln");
    let llib_path = dir.join("my_lib.llib");
    let consumer_bad_path = dir.join("consumer_bad.ln");
    let consumer_good_path = dir.join("consumer_good.ln");

    let provider_src = r#"
module my_lib {
    export fn order_check(longer: &i32, shorter: &i32) requires life(longer) >= life(shorter) {}
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write my_lib.ln");

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
    assert!(res_prov.is_ok(), "Failed to build my_lib.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "my_lib.llib was not created!");

    // Remove source file so resolution relies solely on .llib
    fs::remove_file(&provider_path).expect("Failed to remove my_lib.ln");

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

    let consumer_bad_src = r#"
import "my_lib";

fn main() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    my_lib::order_check(&y, &x);
}
"#;
    fs::write(&consumer_bad_path, consumer_bad_src).expect("Failed to write consumer_bad.ln");
    let res_bad = compile(
        consumer_bad_path.to_str().unwrap(),
        consumer_bad_src.to_string(),
        &opts,
    );
    assert!(res_bad.is_err(), "Binary .llib mode: Inverted argument order MUST fail constraint checking!");
    let errs = res_bad.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "Binary .llib mode: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );

    let consumer_good_src = r#"
import "my_lib";

fn main() {
    dec x: i32 = 1;
    dec y: i32 = 2;
    my_lib::order_check(&x, &y);
}
"#;
    fs::write(&consumer_good_path, consumer_good_src).expect("Failed to write consumer_good.ln");
    let res_good = compile(
        consumer_good_path.to_str().unwrap(),
        consumer_good_src.to_string(),
        &opts,
    );
    assert!(res_good.is_ok(), "Binary .llib mode: Valid argument order MUST pass: {:?}", res_good.err());
}

/// SYNTAX-11: Self identity survives canonicalization.
///
/// source:
///     requires life(self) >= life(other)
/// expected canonical semantic subjects:
///     SelfVal ⪰ Param(i)
/// NOT:
///     Param(0) ⪰ Param(i)
///
/// Also verifies survival across .llib serialization and deserialization.
#[test]
fn test_syntax_11_self_identity_survives_canonicalization() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_11");
    let llib_path = dir.join("self_lib.llib");
    let provider_path = dir.join("self_lib.ln");

    let provider_src = r#"
export trait Checkable {
    fn check_self(self: &Self, other: &i32) requires life(self) >= life(other);
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write self_lib.ln");

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
    assert!(res_prov.is_ok(), "Failed to build self_lib.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "self_lib.llib was not created!");

    // Inspect decoded .llib metadata
    let bytes = fs::read(&llib_path).expect("Failed to read self_lib.llib");
    let mut cursor = std::io::Cursor::new(&bytes);
    let (_, _, _, semantic_opt) = luna_llib::MlibReader::read_module(&mut cursor)
        .expect("Failed to parse self_lib.llib with MlibReader");
    let semantic_meta = semantic_opt.expect("Missing semantic metadata in self_lib.llib");

    let trait_sym = semantic_meta.interface.exported_symbols.get("Checkable").expect("Checkable");
    let method_sym = trait_sym.children.get("check_self").expect("check_self in Checkable");
    let contract = method_sym.lifetime_contract.as_ref().expect("contract on check_self");

    // Crucial Assertions for SYNTAX-11:
    // 1. Constraint count must be exactly 1
    assert_eq!(contract.outlives_constraints.len(), 1);
    let c = &contract.outlives_constraints[0];

    // 2. Longer subject MUST be SelfVal (NOT Param(0))
    assert_eq!(c.longer, luna_semantic::CanonicalContractSubject::SelfVal,
        "SYNTAX-11 violation: longer subject must be SelfVal, got {:?}", c.longer);
    assert_ne!(c.longer, luna_semantic::CanonicalContractSubject::Param(0),
        "SYNTAX-11 violation: longer subject was collapsed to Param(0)!");

    // 3. Shorter subject is Param(0) (the first explicit parameter after receiver)
    assert_eq!(c.shorter, luna_semantic::CanonicalContractSubject::Param(0),
        "shorter subject must be Param(0)");

    // 4. Preconditions generation preserves SelfVal semantic identity
    let preconds = contract.input_preconditions();
    assert_eq!(preconds.len(), 1);
    assert_eq!(preconds[0].0, luna_semantic::region::LifetimeSubject::self_val());
    assert_eq!(preconds[0].1, luna_semantic::region::LifetimeSubject::param(0));
}

/// SYNTAX-12: Surface does not compute transitive closure.
///
/// canonical direct constraints:
///     A ⪰ B
///     B ⪰ C
///
/// RegionSolution:
///     A ⪰ C == true
///
/// Proves that the surface parser/resolver does not eagerly materialize transitive edges into
/// CanonicalLifetimeContract, leaving transitive entailment exclusively to RegionSolution.
#[test]
fn test_syntax_12_surface_does_not_compute_transitive_closure() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("syntax_12");
    let llib_path = dir.join("transitive_lib.llib");
    let provider_path = dir.join("transitive_lib.ln");

    let provider_src = r#"
export fn chain(a: &i32, b: &i32, c: &i32)
    requires
        life(a) >= life(b),
        life(b) >= life(c)
{}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write transitive_lib.ln");

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
    assert!(res_prov.is_ok(), "Failed to build transitive_lib.llib: {:?}", res_prov.err());

    let bytes = fs::read(&llib_path).expect("Failed to read transitive_lib.llib");
    let mut cursor = std::io::Cursor::new(&bytes);
    let (_, _, _, semantic_opt) = luna_llib::MlibReader::read_module(&mut cursor)
        .expect("Failed to parse transitive_lib.llib with MlibReader");
    let semantic_meta = semantic_opt.expect("Missing semantic metadata");

    let chain_sym = semantic_meta.interface.exported_symbols.get("chain")
        .expect("Symbol 'chain' not found in exported symbols");
    let contract = chain_sym.lifetime_contract.as_ref()
        .expect("Symbol 'chain' missing lifetime_contract!");

    // 1. Direct Canonical Constraints: exactly 2 edges (A >= B and B >= C).
    // The surface layer MUST NOT materialize A >= C.
    assert_eq!(contract.outlives_constraints.len(), 2,
        "Surface materialized extra edges! Expected exactly 2 direct constraints, got: {:?}",
        contract.outlives_constraints
    );
    assert_eq!(
        contract.outlives_constraints[0],
        luna_semantic::CanonicalOutlivesConstraint::new(0, 1)
    );
    assert_eq!(
        contract.outlives_constraints[1],
        luna_semantic::CanonicalOutlivesConstraint::new(1, 2)
    );

    // 2. Region Graph & Solution: Transitive entailment (A >= C) is derived exclusively by RegionSolution
    let resolved = luna_semantic::region::contract_gen::ResolvedFunctionContract::from_canonical_preconditions(contract);
    let mut graph = luna_semantic::region::graph::RegionGraph::new();
    let mut bindings = luna_semantic::region::subject::LifetimeRegionBindings::new();
    let r_a = graph.add_region(luna_semantic::region::kind::RegionKind::Contract).unwrap();
    let r_b = graph.add_region(luna_semantic::region::kind::RegionKind::Contract).unwrap();
    let r_c = graph.add_region(luna_semantic::region::kind::RegionKind::Contract).unwrap();
    bindings.bind(luna_semantic::region::LifetimeSubject::param(0), r_a).unwrap();
    bindings.bind(luna_semantic::region::LifetimeSubject::param(1), r_b).unwrap();
    bindings.bind(luna_semantic::region::LifetimeSubject::param(2), r_c).unwrap();

    let generator = luna_semantic::region::contract_gen::ContractConstraintGenerator::new();
    generator.lower_function_contract(
        &mut graph,
        &bindings,
        &resolved,
        false,
        &[],
        false,
        luna_semantic::region::constraint::ConstraintTransport::Source,
        None,
    ).unwrap();

    let solution = luna_semantic::region::solve_region_graph(&graph);
    assert!(solution.outlives(r_a, r_b), "Direct relation A >= B must hold");
    assert!(solution.outlives(r_b, r_c), "Direct relation B >= C must hold");
    assert!(solution.outlives(r_a, r_c), "Transitive entailment A >= C must be derived by RegionSolution!");
    assert!(!solution.outlives(r_c, r_a), "Inverted relation C >= A must NOT hold");
}
