use luna_driver::{compile, CompilerOptions};
use luna_llib::MlibReader;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_lifetime_abi_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

/// Acceptance Test 1: life_from(a) - Single parameter provenance parity.
/// A function returning a reference with `life_from(a)` must tie the returned reference's
/// lifetime exclusively to `a`, leaving `b` independent, across BOTH source (.ln) and binary (.llib).
#[test]
fn test_abi_single_param_provenance_parity() {
    let temp_dir = create_temp_dir("single_param_parity");
    let provider_path = temp_dir.join("math.ln");
    let llib_path = temp_dir.join("math.llib");
    let consumer_conflict_path = temp_dir.join("consumer_conflict.ln");
    let consumer_ok_path = temp_dir.join("consumer_ok.ln");

    let provider_src = r#"
module math {
    export fn choose_first(a: &i32, b: &i32) -> &i32 life_from(a) {
        return a;
    }
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write math.ln");

    // Case 1: Consumer mutates `x` (which is `a`), causing borrowck conflict while `r` is live
    let consumer_conflict_src = r#"
import "math";

fn main() {
    dec rw x = 10;
    dec rw y = 20;
    dec r = math::choose_first(&x, &y);
    x = 30; // Conflict: x is borrowed by r!
    dec _ = *r;
}
"#;
    fs::write(&consumer_conflict_path, consumer_conflict_src).expect("Failed to write consumer_conflict.ln");

    // Case 2: Consumer mutates `y` (which is `b`), which should be SAFE since r only borrows from a
    let consumer_ok_src = r#"
import "math";

fn main() {
    dec rw x = 10;
    dec rw y = 20;
    dec r = math::choose_first(&x, &y);
    y = 30; // Safe: y was not borrowed by r!
    dec _ = *r;
}
"#;
    fs::write(&consumer_ok_path, consumer_ok_src).expect("Failed to write consumer_ok.ln");

    let opts = CompilerOptions {
        output_path: Some(temp_dir.join("dummy.obj").to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        quiet: true,
        search_paths: vec![temp_dir.to_str().unwrap().to_string()],
        no_link: true,
        ..Default::default()
    };

    // --- Phase 1: Source Provider Mode (math.llib does NOT exist) ---
    assert!(!llib_path.exists());

    let res_conflict_src = compile(
        consumer_conflict_path.to_str().unwrap(),
        consumer_conflict_src.to_string(),
        &opts,
    );
    assert!(
        res_conflict_src.is_err(),
        "Source mode: Consumer mutating borrowed 'x' MUST fail borrow checking!"
    );

    let res_ok_src = compile(
        consumer_ok_path.to_str().unwrap(),
        consumer_ok_src.to_string(),
        &opts,
    );
    assert!(
        res_ok_src.is_ok(),
        "Source mode: Consumer mutating unborrowed 'y' MUST pass borrow checking! Got: {:?}",
        res_ok_src.err()
    );

    // --- Phase 2: Compile Provider to math.llib ---
    let prov_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: true,
        search_paths: vec![temp_dir.to_str().unwrap().to_string()],
        no_link: true,
        ..Default::default()
    };
    let res_prov = compile(
        provider_path.to_str().unwrap(),
        provider_src.to_string(),
        &prov_opts,
    );
    assert!(res_prov.is_ok(), "Failed to build math.llib: {:?}", res_prov.err());
    assert!(llib_path.exists(), "math.llib was not created!");

    // Remove source file to guarantee compilation relies solely on math.llib
    fs::remove_file(&provider_path).expect("Failed to remove math.ln");

    // --- Phase 3: Binary .llib Provider Mode (math.llib exists) ---
    let res_conflict_llib = compile(
        consumer_conflict_path.to_str().unwrap(),
        consumer_conflict_src.to_string(),
        &opts,
    );
    assert!(
        res_conflict_llib.is_err(),
        "Binary .llib mode: Consumer mutating borrowed 'x' MUST fail borrow checking!"
    );

    let res_ok_llib = compile(
        consumer_ok_path.to_str().unwrap(),
        consumer_ok_src.to_string(),
        &opts,
    );
    assert!(
        res_ok_llib.is_ok(),
        "Binary .llib mode: Consumer mutating unborrowed 'y' MUST pass borrow checking! Got: {:?}",
        res_ok_llib.err()
    );
}

/// Acceptance Test 2: life_from(a | b) - Multi-parameter provenance parity.
/// A function with `life_from(a | b)` must tie the return reference to BOTH `a` and `b`.
/// Mutating either `a` or `b` while `r` is live must be rejected on BOTH source and .llib.
#[test]
fn test_abi_multi_param_provenance_parity() {
    let temp_dir = create_temp_dir("multi_param_parity");
    let provider_path = temp_dir.join("branch.ln");
    let llib_path = temp_dir.join("branch.llib");
    let consumer_mut_a_path = temp_dir.join("consumer_mut_a.ln");
    let consumer_mut_b_path = temp_dir.join("consumer_mut_b.ln");
    let consumer_safe_path = temp_dir.join("consumer_safe.ln");

    let provider_src = r#"
module branch {
    export fn pick(a: &i32, b: &i32, cond: bool) -> &i32 life_from(a | b) {
        if cond {
            return a;
        }
        return b;
    }
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write branch.ln");

    let consumer_mut_a_src = r#"
import "branch";

fn main() {
    dec rw x = 10;
    dec rw y = 20;
    dec r = branch::pick(&x, &y, true);
    x = 100; // Conflict on x!
    dec _ = *r;
}
"#;
    fs::write(&consumer_mut_a_path, consumer_mut_a_src).expect("Failed to write consumer_mut_a.ln");

    let consumer_mut_b_src = r#"
import "branch";

fn main() {
    dec rw x = 10;
    dec rw y = 20;
    dec r = branch::pick(&x, &y, false);
    y = 200; // Conflict on y!
    dec _ = *r;
}
"#;
    fs::write(&consumer_mut_b_path, consumer_mut_b_src).expect("Failed to write consumer_mut_b.ln");

    let consumer_safe_src = r#"
import "branch";

fn main() {
    dec rw x = 10;
    dec rw y = 20;
    dec r = branch::pick(&x, &y, true);
    dec _ = *r;
    x = 100; // Safe: r is no longer live after this line
    y = 200;
}
"#;
    fs::write(&consumer_safe_path, consumer_safe_src).expect("Failed to write consumer_safe.ln");

    let opts = CompilerOptions {
        output_path: Some(temp_dir.join("dummy.obj").to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        quiet: true,
        search_paths: vec![temp_dir.to_str().unwrap().to_string()],
        no_link: true,
        ..Default::default()
    };

    // --- Source Mode ---
    assert!(compile(consumer_mut_a_path.to_str().unwrap(), consumer_mut_a_src.to_string(), &opts).is_err());
    assert!(compile(consumer_mut_b_path.to_str().unwrap(), consumer_mut_b_src.to_string(), &opts).is_err());
    let res_safe_src = compile(consumer_safe_path.to_str().unwrap(), consumer_safe_src.to_string(), &opts);
    if let Err(e) = &res_safe_src {
        println!("Safe src failed with errors: {:?}", e);
    }
    assert!(res_safe_src.is_ok());

    // --- Compile to .llib ---
    let prov_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: true,
        search_paths: vec![temp_dir.to_str().unwrap().to_string()],
        no_link: true,
        ..Default::default()
    };
    let res_prov = compile(provider_path.to_str().unwrap(), provider_src.to_string(), &prov_opts);
    assert!(res_prov.is_ok(), "Failed to compile branch.llib: {:?}", res_prov.err());

    fs::remove_file(&provider_path).expect("Failed to remove branch.ln");

    // --- Binary .llib Mode ---
    assert!(compile(consumer_mut_a_path.to_str().unwrap(), consumer_mut_a_src.to_string(), &opts).is_err());
    assert!(compile(consumer_mut_b_path.to_str().unwrap(), consumer_mut_b_src.to_string(), &opts).is_err());
    assert!(compile(consumer_safe_path.to_str().unwrap(), consumer_safe_src.to_string(), &opts).is_ok());
}

/// Acceptance Test 3: Binary .llib metadata inspection.
/// Directly deserializes the .llib archive to ensure the CanonicalLifetimeContract
/// is faithfully stored in ExportedSymbol metadata with valid version and provenance.
#[test]
fn test_abi_metadata_structure_inspection() {
    let temp_dir = create_temp_dir("metadata_inspection");
    let provider_path = temp_dir.join("geom.ln");
    let llib_path = temp_dir.join("geom.llib");

    let provider_src = r#"
export fn choose(a: &i32, b: &i32) -> &i32 life_from(a) {
    return a;
}

export fn pick(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    return a;
}
"#;
    fs::write(&provider_path, provider_src).expect("Failed to write geom.ln");

    let prov_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: true,
        search_paths: vec![temp_dir.to_str().unwrap().to_string()],
        no_link: true,
        ..Default::default()
    };
    assert!(compile(provider_path.to_str().unwrap(), provider_src.to_string(), &prov_opts).is_ok());

    let bytes = fs::read(&llib_path).expect("Failed to read geom.llib");
    let mut cursor = std::io::Cursor::new(&bytes);
    let (_, _, _, semantic_opt) = MlibReader::read_module(&mut cursor).expect("Failed to parse geom.llib with MlibReader");
    let semantic_meta = semantic_opt.expect("Missing semantic metadata in geom.llib");

    // Inspect 'choose' symbol
    let choose_sym = semantic_meta.interface.exported_symbols.get("choose")
        .expect("Symbol 'choose' not found in geom.llib exports");
    let choose_contract = choose_sym.lifetime_contract.as_ref()
        .expect("Symbol 'choose' is missing lifetime_contract in .llib!");
    assert_eq!(choose_contract.version, luna_semantic::LIFETIME_RELATION_ABI_VERSION);
    assert_eq!(
        choose_contract.return_provenance,
        Some(luna_semantic::CanonicalProvenance::Param(0))
    );

    // Inspect 'pick' symbol
    let pick_sym = semantic_meta.interface.exported_symbols.get("pick")
        .expect("Symbol 'pick' not found in geom.llib exports");
    let pick_contract = pick_sym.lifetime_contract.as_ref()
        .expect("Symbol 'pick' is missing lifetime_contract in .llib!");
    assert_eq!(pick_contract.version, luna_semantic::LIFETIME_RELATION_ABI_VERSION);
    assert_eq!(
        pick_contract.return_provenance,
        Some(luna_semantic::CanonicalProvenance::ParamSet(vec![0, 1]))
    );
}
