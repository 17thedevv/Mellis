use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_struct_lifetime_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn make_opts(test_sysroot: &Sysroot, extra_search_path: Option<&str>) -> CompilerOptions {
    let mut search_paths = vec![test_sysroot.root().to_string_lossy().to_string()];
    if let Some(p) = extra_search_path {
        search_paths.insert(0, p.to_string());
    }
    CompilerOptions {
        search_paths,
        quiet: true,
        ..Default::default()
    }
}

/// STRUCT-LIFE-01: life(field) >= life(self) parses and resolves cleanly.
#[test]
fn test_struct_life_01_parse_and_resolve() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_01");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "STRUCT-LIFE-01: Struct contract must parse and resolve cleanly: {:?}", res.err());
}

/// STRUCT-LIFE-02: Field identity canonicalized using 0-based field index/path, not field name string.
#[test]
fn test_struct_life_02_field_identity_canonicalized() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_02");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Pair {
    first: i32,
    second: &i32
} requires life(second) >= life(self);

fn main() {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "STRUCT-LIFE-02: Field identity canonicalization must succeed: {:?}", res.err());
}

/// STRUCT-LIFE-03: Valid construction with provenance longer than container accepted.
#[test]
fn test_struct_life_03_valid_construction() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_03");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec x: i32 = 10;
    dec h: Holder = Holder { value: &x };
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "STRUCT-LIFE-03: Valid construction must be accepted: {:?}", res.err());
}

/// STRUCT-LIFE-04: Reject field provenance shorter than container with E2016.
#[test]
fn test_struct_life_04_reject_shorter_provenance() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_04");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec base: i32 = 1;
    dec rw h: Holder = Holder { value: &base };
    {
        dec x: i32 = 10;
        h = Holder { value: &x };
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-04: Shorter provenance must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-04: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-05: Moving aggregate to longer-lived carrier revalidates against destination instance region.
/// (valid into short carrier, invalid into longer carrier; proves dynamic destination identity).
#[test]
fn test_struct_life_05_move_to_longer_lived_carrier() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_05");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec base: i32 = 1;
    dec rw long_carrier: Holder = Holder { value: &base };
    {
        dec x: i32 = 10;
        dec short_carrier: Holder = Holder { value: &x };
        long_carrier = short_carrier;
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-05: Move to longer carrier must fail dynamic instance check");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-05: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-06: Move to shorter/equal carrier valid when relation holds.
#[test]
fn test_struct_life_06_move_to_shorter_carrier() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_06");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec x: i32 = 10;
    dec h1: Holder = Holder { value: &x };
    {
        dec h2: Holder = h1;
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "STRUCT-LIFE-06: Move to shorter carrier must be accepted: {:?}", res.err());
}

/// STRUCT-LIFE-07: Joined field provenance checked universally/conservatively across CFG paths.
#[test]
fn test_struct_life_07_joined_field_provenance_cfg() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_07");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec rw h: Holder;
    dec long_val: i32 = 100;
    dec cond: bool = true;
    if cond {
        h = Holder { value: &long_val };
    } else {
        dec short_val: i32 = 200;
        h = Holder { value: &short_val };
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-07: Joined field provenance with short-lived branch must fail");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-07: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-08: Two instances of same type have independent dynamic self regions.
#[test]
fn test_struct_life_08_independent_self_regions() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_08");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec x: i32 = 10;
    dec h1: Holder = Holder { value: &x };
    {
        dec y: i32 = 20;
        dec h2: Holder = Holder { value: &y };
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "STRUCT-LIFE-08: Independent instances must succeed: {:?}", res.err());
}

/// STRUCT-LIFE-09: Source vs .llib metadata and obligation parity.
#[test]
fn test_struct_life_09_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_09");
    let llib_path = dir.join("holder_lib.llib");
    let prov_path = dir.join("holder_lib.ln");

    let prov_src = r#"
module holder_lib {
    export struct Holder {
        value: &i32
    } requires life(value) >= life(self);
}
"#;
    fs::write(&prov_path, prov_src).unwrap();

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
    let res_prov = compile(prov_path.to_str().unwrap(), prov_src.to_string(), &prov_opts);
    assert!(res_prov.is_ok(), "STRUCT-LIFE-09: Failed to build holder_lib.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "STRUCT-LIFE-09: holder_lib.llib was not created!");

    fs::remove_file(&prov_path).unwrap();

    let consumer_opts = CompilerOptions {
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

    // 1. Consumer good
    let good_src = r#"
import "holder_lib";

fn main() {
    dec x: i32 = 10;
    dec h: holder_lib::Holder = holder_lib::Holder { value: &x };
}
"#;
    let good_path = dir.join("consumer_good.ln");
    fs::write(&good_path, good_src).unwrap();
    let res_good = compile(good_path.to_str().unwrap(), good_src.to_string(), &consumer_opts);
    assert!(res_good.is_ok(), "STRUCT-LIFE-09: Binary .llib valid construction must pass: {:?}", res_good.err());

    // 2. Consumer bad
    let bad_src = r#"
import "holder_lib";

fn main() {
    dec base: i32 = 1;
    dec rw h: holder_lib::Holder = holder_lib::Holder { value: &base };
    {
        dec x: i32 = 10;
        h = holder_lib::Holder { value: &x };
    }
}
"#;
    let bad_path = dir.join("consumer_bad.ln");
    fs::write(&bad_path, bad_src).unwrap();
    let res_bad = compile(bad_path.to_str().unwrap(), bad_src.to_string(), &consumer_opts);
    assert!(res_bad.is_err(), "STRUCT-LIFE-09: Binary .llib invalid construction must fail");
    let errs = res_bad.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-09: Expected E2016 LifetimeConstraintViolation in .llib mode, got: {:?}", errs
    );
}

/// STRUCT-LIFE-10: Function projection life(holder.field) remains explicitly unsupported in v1.
#[test]
fn test_struct_life_10_function_projection_unsupported() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_10");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn test(h: &Holder) requires life(h.value) >= life(h) {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-10: Function projection must be rejected");
}

/// STRUCT-LIFE-11: Field storage is not referent validity.
#[test]
fn test_struct_life_11_field_storage_not_referent_validity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_11");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec base: i32 = 100;
    dec rw h: Holder = Holder { value: &base };
    {
        dec temp: i32 = 42;
        h.value = &temp;
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-11: Assigning short reference to field must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-11: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-12: Reverse relation SelfVal >= Field is explicitly unsupported in v1.
#[test]
fn test_struct_life_12_reverse_relation_unsupported() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_12");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(self) >= life(value);

fn main() {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-12: Reverse relation SelfVal >= Field must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("unsupported lifetime contract relation") || d.message.contains("only admits")),
        "STRUCT-LIFE-12: Expected unsupported contract relation error, got: {:?}", errs
    );
}

/// STRUCT-LIFE-13: Non-reference field used in life(field) is semantically rejected.
#[test]
fn test_struct_life_13_non_reference_field_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_13");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct BadHolder {
    value: i32
} requires life(value) >= life(self);

fn main() {}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-13: Non-reference field must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("non-reference field")),
        "STRUCT-LIFE-13: Expected non-reference field error with E2016, got: {:?}", errs
    );
}

/// STRUCT-LIFE-14: Aliased FieldPtr update preserves weak-update/provenance semantics.
#[test]
fn test_struct_life_14_aliased_field_ptr_uses_place() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_14");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec x: i32 = 10;
    dec rw h: Holder = Holder { value: &x };
    dec p: &rw Holder = &rw h;
    {
        dec short_val: i32 = 20;
        p.value = &short_val;
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-14: Aliased field pointer update must revalidate against container");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-14: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-15: Return-by-value revalidation occurs at caller destination carrier establishment.
#[test]
fn test_struct_life_15_return_by_value_caller_revalidation() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_15");
    let opts = make_opts(&test_sysroot, None);

    // 1. Good caller: return stored into carrier living no longer than argument
    let good_src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn make(p: &i32) -> Holder life_from(p) {
    dec h: Holder = Holder { value: p };
    return h;
}

fn caller_good() {
    dec x: i32 = 10;
    dec h: Holder = make(&x);
}
"#;
    let good_path = dir.join("good.ln");
    fs::write(&good_path, good_src).unwrap();
    let res_good = check(good_path.to_str().unwrap(), good_src.to_string(), &opts);
    assert!(res_good.is_ok(), "STRUCT-LIFE-15: Valid caller establishment must pass: {:?}", res_good.err());

    // 2. Bad caller: return stored into longer carrier with short argument
    let bad_src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn make(p: &i32) -> Holder life_from(p) {
    dec h: Holder = Holder { value: p };
    return h;
}

fn caller_bad() {
    dec base: i32 = 1;
    dec rw long_h: Holder = Holder { value: &base };
    {
        dec local: i32 = 10;
        long_h = make(&local);
    }
}
"#;
    let bad_path = dir.join("bad.ln");
    fs::write(&bad_path, bad_src).unwrap();
    let res_bad = check(bad_path.to_str().unwrap(), bad_src.to_string(), &opts);
    assert!(res_bad.is_err(), "STRUCT-LIFE-15: Returning into longer carrier must fail with E2016");
    let errs = res_bad.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-15: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-16: Missing provenance/instance Region mapping produces Incomplete(ShadowGap), never Valid.
#[test]
fn test_struct_life_16_missing_provenance_shadow_gap() {
    use luna_borrowck::borrow_analysis::BorrowStateData;
    use luna_borrowck::region_bridge::{RegionBorrowBridge, RegionBorrowContext, ShadowRegionVerdict};
    use luna_semantic::region::{solve_region_graph, realize_regions};
    use luna_semantic::{CanonicalTypeLifetimeContract, CanonicalFieldPath};
    use luna_mvir::{Function, GlobalId, Operand, ValueId, ValueData, Instruction, ValueOrigin};

    let func = Function {
        name: GlobalId {
            name: "test".to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: luna_semantic::SemanticTypeId(0),
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        values: vec![ValueData {
            inst: Instruction::Alloca,
            ty: luna_semantic::SemanticTypeId(0),
            span: None,
            origin: ValueOrigin::Local,
        }],
        blocks: Vec::new(),
    };
    let loop_info = luna_borrowck::cfg::analyze_loops(&func);
    let context = RegionBorrowContext::build(&func, &loop_info, None);
    let solution = solve_region_graph(&context.graph);
    let realization = realize_regions(&solution, &context.facts).unwrap();
    let bridge = RegionBorrowBridge::new(&context, &solution, &realization);

    let contract = CanonicalTypeLifetimeContract::new(
        vec![luna_semantic::CanonicalTypeOutlivesConstraint {
            longer: luna_semantic::CanonicalTypeLifetimeSubject::Field(CanonicalFieldPath::single(0)),
            shorter: luna_semantic::CanonicalTypeLifetimeSubject::SelfVal,
        }],
    );
    let state = BorrowStateData::default(); // empty provenance

    let verdict = bridge.check_type_contract_instance(
        &contract,
        &Operand::Value(ValueId(0)),
        &Operand::Value(ValueId(0)),
        &state,
        None,
    );
    assert!(
        matches!(verdict, ShadowRegionVerdict::Incomplete(_)),
        "STRUCT-LIFE-16: Empty provenance must produce Incomplete, never Valid: {:?}", verdict
    );
}

/// STRUCT-LIFE-17: .llib metadata schema compatibility/version behavior is deterministic.
#[test]
fn test_struct_life_17_llib_version_compatibility() {
    use luna_llib::format::{LlibHeader, LLIB_FORMAT_VERSION};
    use luna_llib::MlibError;
    use luna_llib::reader::MlibReader;
    use luna_semantic::CanonicalTypeLifetimeContract;
    use std::io::Cursor;

    assert_eq!(CanonicalTypeLifetimeContract::CURRENT_VERSION, 1, "STRUCT-LIFE-17: Contract schema version must be 1");
    assert_eq!(LLIB_FORMAT_VERSION, 1, "STRUCT-LIFE-17: Current LLIB_FORMAT_VERSION is 1");

    // An artifact with an incompatible version (e.g. 999) must be rejected with VersionMismatch
    let mut header = LlibHeader::new();
    header.format_version = 999;
    let mut buffer = Vec::new();
    header.write_to(&mut buffer).unwrap();
    let mut cursor = Cursor::new(&buffer);
    let reader_res = MlibReader::read_manifest(&mut cursor);
    assert!(reader_res.is_err(), "STRUCT-LIFE-17: Incompatible version must be rejected by MlibReader");
    match reader_res.err().unwrap() {
        MlibError::VersionMismatch(found) => {
            assert_eq!(found, 999);
        }
        other => panic!("STRUCT-LIFE-17: Expected VersionMismatch, got {:?}", other),
    }
}

/// STRUCT-LIFE-18: Post-construction field mutation revalidates invariant.
#[test]
fn test_struct_life_18_post_construction_field_mutation() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_18");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct Holder {
    value: &i32
} requires life(value) >= life(self);

fn main() {
    dec long_val: i32 = 10;
    dec rw h: Holder = Holder { value: &long_val };
    {
        dec short_val: i32 = 20;
        h.value = &short_val;
    }
}
"#;
    let path = dir.join("test.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-18: Post-construction field mutation with short ref must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") || d.message.contains("LifetimeConstraintViolation")),
        "STRUCT-LIFE-18: Expected E2016 LifetimeConstraintViolation, got: {:?}", errs
    );
}

/// STRUCT-LIFE-19: Provenance survives independently of loan state.
/// A field retains the same referent provenance while the associated access-loan state changes/ends.
/// Expected: type-contract provenance resolution is unchanged.
#[test]
fn test_struct_life_19_provenance_survives_loan_state_change() {
    use luna_borrowck::borrow_analysis::{BorrowStateData, Loan, PlaceDesc, ProvenanceSet};
    use luna_borrowck::region_bridge::{RegionBorrowBridge, RegionBorrowContext, ShadowRegionVerdict};
    use luna_semantic::region::{solve_region_graph, realize_regions};
    use luna_semantic::{CanonicalFieldPath, CanonicalTypeLifetimeContract, SemanticTypeId};
    use luna_mvir::{Function, GlobalId, Instruction, Operand, ValueData, ValueId, ValueOrigin};
    use std::collections::HashSet;

    let func = Function {
        name: GlobalId {
            name: "test_life_19".to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        values: vec![
            ValueData {
                inst: Instruction::Alloca,
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
            ValueData {
                inst: Instruction::Alloca,
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
        ],
        blocks: vec![luna_mvir::BasicBlock {
            label: luna_mvir::LabelId { name: "b0".to_string() },
            insts: vec![ValueId(0), ValueId(1)],
            terminator: None,
        }],
    };
    let loop_info = luna_borrowck::cfg::analyze_loops(&func);
    let context = RegionBorrowContext::build(&func, &loop_info, None);
    let solution = solve_region_graph(&context.graph);
    let realization = realize_regions(&solution, &context.facts).unwrap();
    let bridge = RegionBorrowBridge::new(&context, &solution, &realization);

    let contract = CanonicalTypeLifetimeContract::new(
        vec![luna_semantic::CanonicalTypeOutlivesConstraint {
            longer: luna_semantic::CanonicalTypeLifetimeSubject::Field(CanonicalFieldPath::single(0)),
            shorter: luna_semantic::CanonicalTypeLifetimeSubject::SelfVal,
        }],
    );

    let holder_place = PlaceDesc {
        root: Operand::Value(ValueId(0)),
        projections: vec![luna_borrowck::borrow_analysis::Projection::Field(0)],
    };
    let referent_source = PlaceDesc {
        root: Operand::Value(ValueId(1)),
        projections: Vec::new(),
    };

    let mut state = BorrowStateData::default();
    let mut sources = HashSet::new();
    sources.insert(referent_source);
    state.field_provenance.insert(holder_place, ProvenanceSet { sources });

    // 1. Initially, there is an active loan capability over the referent place
    let loan = Loan {
        id: ValueId(1),
        place: Operand::Value(ValueId(1)),
        is_rw: false,
    };
    state.direct_provenance.entry(ValueId(1)).or_default().insert(loan.clone());
    state.carried_provenance.entry(ValueId(0)).or_default().insert(loan);

    // Initial check: provenance resolves to ValueId(1)'s region
    let regions_before = bridge
        .resolve_field_dependency_regions(&Operand::Value(ValueId(0)), &CanonicalFieldPath::single(0), &state)
        .expect("Provenance resolution must succeed before loan state change");

    let verdict_before = bridge.check_type_contract_instance(
        &contract,
        &Operand::Value(ValueId(0)),
        &Operand::Value(ValueId(0)),
        &state,
        None,
    );
    assert_eq!(verdict_before, ShadowRegionVerdict::Valid);

    // 2. Loan state changes/ends: clear active access loans
    state.direct_provenance.clear();
    state.carried_provenance.clear();
    assert!(state.direct_provenance.is_empty());
    assert!(state.carried_provenance.is_empty());

    // 3. Re-check: field_provenance survives independently of loan state
    let regions_after = bridge
        .resolve_field_dependency_regions(&Operand::Value(ValueId(0)), &CanonicalFieldPath::single(0), &state)
        .expect("Provenance resolution must succeed after access loans ended");

    assert_eq!(
        regions_before, regions_after,
        "STRUCT-LIFE-19: Type-contract provenance resolution must be unchanged after loan state ends"
    );

    let verdict_after = bridge.check_type_contract_instance(
        &contract,
        &Operand::Value(ValueId(0)),
        &Operand::Value(ValueId(0)),
        &state,
        None,
    );
    assert_eq!(
        verdict_after,
        ShadowRegionVerdict::Valid,
        "STRUCT-LIFE-19: Type contract remains Valid even when active loans are empty"
    );
}

/// STRUCT-LIFE-20: Alias identity is Place-based.
/// Create two distinct SSA/ValueIds that denote the same projected Place.
/// Update through one alias.
/// Expected:
/// the other observation sees the same updated field provenance.
/// No semantic state is split merely because ValueIds differ.
#[test]
fn test_struct_life_20_alias_identity_is_place_based() {
    use luna_borrowck::borrow_analysis::{BorrowStateData, PlaceDesc, Projection, ProvenanceSet};
    use luna_borrowck::region_bridge::{RegionBorrowBridge, RegionBorrowContext};
    use luna_semantic::region::{solve_region_graph, realize_regions};
    use luna_semantic::{CanonicalFieldPath, SemanticTypeId};
    use luna_mvir::{Function, GlobalId, Instruction, Operand, ValueData, ValueId, ValueOrigin};
    use std::collections::HashSet;

    // Create a function with:
    // ValueId(0) = Alloca (Holder struct)
    // ValueId(1) = FieldPtr { base: ValueId(0), field_idx: 0 } (alias 1)
    // ValueId(2) = FieldPtr { base: ValueId(0), field_idx: 0 } (alias 2, distinct SSA ValueId)
    // ValueId(3) = Alloca (referent place x)
    let func = Function {
        name: GlobalId {
            name: "test_life_20".to_string(),
            symbol_id: None,
        },
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        values: vec![
            ValueData {
                inst: Instruction::Alloca,
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
            ValueData {
                inst: Instruction::FieldPtr {
                    base: Operand::Value(ValueId(0)),
                    field_idx: 0,
                },
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
            ValueData {
                inst: Instruction::FieldPtr {
                    base: Operand::Value(ValueId(0)),
                    field_idx: 0,
                },
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
            ValueData {
                inst: Instruction::Alloca,
                ty: SemanticTypeId(0),
                span: None,
                origin: ValueOrigin::Local,
            },
        ],
        blocks: vec![luna_mvir::BasicBlock {
            label: luna_mvir::LabelId { name: "b0".to_string() },
            insts: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            terminator: None,
        }],
    };

    assert_ne!(ValueId(1), ValueId(2), "Aliases must have distinct SSA ValueIds");

    let loop_info = luna_borrowck::cfg::analyze_loops(&func);
    let context = RegionBorrowContext::build(&func, &loop_info, None);
    let solution = solve_region_graph(&context.graph);
    let realization = realize_regions(&solution, &context.facts).unwrap();
    let bridge = RegionBorrowBridge::new(&context, &solution, &realization);

    // Both aliases must canonicalize to the exact same PlaceDesc
    let place_desc_1 = bridge.compute_place_desc(&Operand::Value(ValueId(1)));
    let place_desc_2 = bridge.compute_place_desc(&Operand::Value(ValueId(2)));
    assert_eq!(
        place_desc_1, place_desc_2,
        "STRUCT-LIFE-20: Both aliases must resolve to the identical PlaceDesc"
    );
    assert_eq!(
        place_desc_1,
        PlaceDesc {
            root: Operand::Value(ValueId(0)),
            projections: vec![Projection::Field(0)],
        }
    );

    let mut state = BorrowStateData::default();

    // Update field provenance through alias 1 (ValueId(1))
    let referent_place = bridge.compute_place_desc(&Operand::Value(ValueId(3)));
    let mut sources = HashSet::new();
    sources.insert(referent_place.clone());
    state.field_provenance.insert(place_desc_1.clone(), ProvenanceSet { sources });

    // Observe through alias 2 (ValueId(2))
    let observed_via_alias2 = state.field_provenance.get(&place_desc_2);
    assert!(
        observed_via_alias2.is_some(),
        "STRUCT-LIFE-20: Observation through alias 2 must find the updated field provenance"
    );
    let sources_alias2 = &observed_via_alias2.unwrap().sources;
    assert!(
        sources_alias2.contains(&referent_place),
        "STRUCT-LIFE-20: Alias 2 must observe the exact referent place written via alias 1"
    );

    // Observe through aggregate place + field path
    let regions = bridge
        .resolve_field_dependency_regions(
            &Operand::Value(ValueId(0)),
            &CanonicalFieldPath::single(0),
            &state,
        )
        .expect("Aggregate query must resolve the same field provenance");
    let reg_x = context.val_to_region.get(&ValueId(3)).copied().unwrap();
    assert!(regions.contains(&reg_x));
}

/// STRUCT-LIFE-21: Raw pointer is not a safe lifetime subject.
/// struct RawHolder { p: *i32 } requires life(p) >= life(self);
/// Expected: semantic rejection (E2016); no implicit provenance/lifetime relation.
#[test]
fn test_struct_life_21_raw_pointer_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("life_21");
    let opts = make_opts(&test_sysroot, None);

    let src = r#"
struct RawHolder {
    p: *i32
} requires life(p) >= life(self);
"#;
    let path = dir.join("raw_holder.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "STRUCT-LIFE-21: Raw pointer in life(field) must be semantically rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E2016") && d.message.contains("non-reference type")),
        "STRUCT-LIFE-21: Expected E2016 non-reference type error, got: {:?}", errs
    );
}


