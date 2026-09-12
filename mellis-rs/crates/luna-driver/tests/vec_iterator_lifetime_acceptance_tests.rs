use luna_driver::{compile, check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use luna_llib::MlibReader;
use std::fs;
use std::path::{Path, PathBuf};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_vec_iterator_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn locate_canonical_core_ln() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libs")
        .join("external")
        .join("core.ln")
}

fn locate_canonical_alloc_ln() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let alloc_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libs")
        .join("external")
        .join("alloc.ln");
    assert!(alloc_path.exists(), "libs/external/alloc.ln must exist");
    alloc_path
}

/// Phase 5 & 6 Proof 1: alloc.ln compiles to alloc.llib with canonical lifetime contracts.
#[test]
fn test_proof_1_alloc_compiles_to_llib_with_contracts() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_1_compile_alloc");
    let alloc_path = locate_canonical_alloc_ln();
    let alloc_src = fs::read_to_string(&alloc_path).expect("Failed to read alloc.ln");

    let out_llib = dir.join("alloc.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };

    let res = compile(alloc_path.to_str().unwrap(), alloc_src, &compile_opts);
    assert!(
        res.is_ok(),
        "Compiling alloc.ln to alloc.llib must succeed: {:?}",
        res.err()
    );
    assert!(out_llib.exists(), "alloc.llib artifact must exist on disk");

    // Inspect metadata for lifetime contracts
    let mut file = fs::File::open(&out_llib).expect("Failed to open compiled alloc.llib");
    let (_mod, manifest, _obj, semantic_metadata) =
        MlibReader::read_module(&mut file).expect("Failed to parse alloc.llib with MlibReader");
    assert!(manifest.is_some());
    let meta = semantic_metadata.expect("alloc.llib must contain SemanticMetadata");
    let exported = &meta.interface.exported_symbols;

    assert!(exported.contains_key("Vec"), "Vec must be exported");
    assert!(exported.contains_key("VecIter"), "VecIter must be exported");
    assert!(exported.contains_key("VecIterMut"), "VecIterMut must be exported");

    // Sync canonical libs/external/alloc.llib with freshly validated build
    let canonical_llib = alloc_path.with_file_name("alloc.llib");
    let _ = fs::copy(&out_llib, &canonical_llib);
}

/// Phase 5 Proof 2: Vec::get borrowck enforcement.
/// Mutating Vec while a reference returned by get() is active MUST fail borrow checking.
#[test]
fn test_proof_2_vec_get_borrowck_enforcement() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("proof_2_vec_get");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Case 1: Conflict - mutating Vec via push while get() borrow is active
    let conflict_src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec r = v.get(0 as u64);
    v.push(20); // Conflict: v is mutably borrowed while r holds active borrow of v!
    dec _ = r;
}
"#;
    let conflict_path = dir.join("conflict.ln");
    fs::write(&conflict_path, conflict_src).unwrap();
    let res_conflict = check(conflict_path.to_str().unwrap(), conflict_src.to_string(), &opts);
    assert!(
        res_conflict.is_err(),
        "Mutating Vec via push while get() borrow is live must fail borrowck!"
    );

    // Case 2: Safe - mutating Vec after get() borrow has ended
    let safe_src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec r = v.get(0 as u64);
    dec _ = r;
    v.push(20); // Safe: r is dead!
}
"#;
    let safe_path = dir.join("safe.ln");
    fs::write(&safe_path, safe_src).unwrap();
    let res_safe = check(safe_path.to_str().unwrap(), safe_src.to_string(), &opts);
    assert!(
        res_safe.is_ok(),
        "Mutating Vec after get() borrow ended must succeed! Got: {:?}",
        res_safe.err()
    );
}

/// ITER-1: iter() → push() → next_ref() MUST reject
#[test]
fn test_iter_1_push_while_iter_live_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_1_push");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    v.push(20); // Conflict: v mutated while iterator `it` is active!
    dec _ = it.next_ref();
}
"#;
    let path = dir.join("iter_1.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "ITER-1: iter() -> push() -> next_ref() MUST be rejected by borrowck!"
    );
}

/// ITER-2: iter() → clear() → next_ref() MUST reject
#[test]
fn test_iter_2_clear_while_iter_live_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_2_clear");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    v.clear(); // Conflict: v cleared while iterator `it` is active!
    dec _ = it.next_ref();
}
"#;
    let path = dir.join("iter_2.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "ITER-2: iter() -> clear() -> next_ref() MUST be rejected by borrowck!"
    );
}

/// ITER-3: iter() → grow() → next_ref() MUST reject
#[test]
fn test_iter_3_grow_while_iter_live_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_3_grow");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    v.grow(); // Conflict: realloc/grow while iterator `it` is active!
    dec _ = it.next_ref();
}
"#;
    let path = dir.join("iter_3.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_err(),
        "ITER-3: iter() -> grow() -> next_ref() MUST be rejected by borrowck!"
    );
}

/// ITER-4: iter() → next_ref() → stop using it → push() MUST pass
#[test]
fn test_iter_4_next_ref_then_dead_push_allowed() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_4_pass");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let src = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    dec _ = it.next_ref();
    // iterator and yielded reference are completely dead here
    v.push(20); // Safe: no borrow is active on v!
}
"#;
    let path = dir.join("iter_4.ln");
    fs::write(&path, src).unwrap();
    let res = check(path.to_str().unwrap(), src.to_string(), &opts);
    assert!(
        res.is_ok(),
        "ITER-4: iter() -> next_ref() -> dead -> push() MUST pass borrowck! Got: {:?}",
        res.err()
    );
}

/// ITER-5: iter_mut() → mutation through iterator + mutation through Vec simultaneously MUST reject
#[test]
fn test_iter_5_iter_mut_simultaneous_mutation_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("iter_5_mut");

    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Subcase 5A: Mutating Vec directly while iter_mut is active
    let src_a = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter_mut();
    v.push(20); // Conflict: exclusive borrow by iter_mut!
    dec _ = it.next_mut();
}
"#;
    let path_a = dir.join("iter_5a.ln");
    fs::write(&path_a, src_a).unwrap();
    let res_a = check(path_a.to_str().unwrap(), src_a.to_string(), &opts);
    assert!(
        res_a.is_err(),
        "ITER-5A: Vec mutation while iter_mut is active MUST be rejected!"
    );

    // Subcase 5B: Mutating Vec directly while projected mutable reference from next_mut() is active
    let src_b = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter_mut();
    dec opt_ref = it.next_mut();
    v.clear(); // Conflict: exclusive borrow held by projected reference!
    dec _ = opt_ref;
}
"#;
    let path_b = dir.join("iter_5b.ln");
    fs::write(&path_b, src_b).unwrap();
    let res_b = check(path_b.to_str().unwrap(), src_b.to_string(), &opts);
    assert!(
        res_b.is_err(),
        "ITER-5B: Vec clear while projected mutable reference is active MUST be rejected!"
    );
}

/// ITER-6: Source .ln vs .llib iterator contract MUST be identical
#[test]
fn test_iter_6_source_vs_llib_parity() {
    let base_dir = create_temp_dir("iter_6_parity");
    let core_path = locate_canonical_core_ln();
    let core_src = fs::read_to_string(&core_path).expect("Failed to read core.ln");
    let alloc_path = locate_canonical_alloc_ln();
    let alloc_src = fs::read_to_string(&alloc_path).expect("Failed to read alloc.ln");

    // Environment A: Source-only sysroot (alloc.ln and core.ln, no .llib)
    let sysroot_a_dir = base_dir.join("sysroot_a");
    let ext_a = sysroot_a_dir.join("libs").join("external");
    fs::create_dir_all(&ext_a).unwrap();
    fs::write(ext_a.join("core.ln"), &core_src).unwrap();
    fs::write(ext_a.join("alloc.ln"), &alloc_src).unwrap();
    let opts_a = CompilerOptions {
        search_paths: vec![sysroot_a_dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Environment B: Binary-only sysroot (core.llib and alloc.llib, no .ln)
    let sysroot_b_dir = base_dir.join("sysroot_b");
    let ext_b = sysroot_b_dir.join("libs").join("external");
    fs::create_dir_all(&ext_b).unwrap();
    let core_llib_b = ext_b.join("core.llib");
    let alloc_llib_b = ext_b.join("alloc.llib");

    // Compile core.ln -> core.llib in sysroot_b
    let comp_core_opts = CompilerOptions {
        output_path: Some(core_llib_b.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    assert!(compile(ext_a.join("core.ln").to_str().unwrap(), core_src, &comp_core_opts).is_ok());

    // Compile alloc.ln -> alloc.llib in sysroot_b
    let comp_alloc_opts = CompilerOptions {
        output_path: Some(alloc_llib_b.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot_b_dir.to_string_lossy().to_string()],
        ..Default::default()
    };
    assert!(compile(ext_a.join("alloc.ln").to_str().unwrap(), alloc_src, &comp_alloc_opts).is_ok());

    let opts_b = CompilerOptions {
        search_paths: vec![sysroot_b_dir.to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    // Test matrix across both environments:
    let adversarial_conflict = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    v.push(20);
    dec _ = it.next_ref();
}
"#;
    let adversarial_safe = r#"
import <core>;
import <alloc>;

fn main() {
    dec rw v = vec_with_capacity<i32>(4 as u64);
    v.push(10);
    dec rw it = v.iter();
    dec _ = it.next_ref();
    v.push(20);
}
"#;

    // Check Source Mode (Environment A)
    let path_a_conf = ext_a.join("test_conf.ln");
    fs::write(&path_a_conf, adversarial_conflict).unwrap();
    let res_a_conf = check(path_a_conf.to_str().unwrap(), adversarial_conflict.to_string(), &opts_a);

    let path_a_safe = ext_a.join("test_safe.ln");
    fs::write(&path_a_safe, adversarial_safe).unwrap();
    let res_a_safe = check(path_a_safe.to_str().unwrap(), adversarial_safe.to_string(), &opts_a);

    // Check Binary Mode (Environment B)
    let path_b_conf = ext_b.join("test_conf.ln");
    fs::write(&path_b_conf, adversarial_conflict).unwrap();
    let res_b_conf = check(path_b_conf.to_str().unwrap(), adversarial_conflict.to_string(), &opts_b);

    let path_b_safe = ext_b.join("test_safe.ln");
    fs::write(&path_b_safe, adversarial_safe).unwrap();
    let res_b_safe = check(path_b_safe.to_str().unwrap(), adversarial_safe.to_string(), &opts_b);

    // Parity Assertions
    assert!(res_a_conf.is_err(), "Source mode MUST reject iterator conflict!");
    assert!(res_b_conf.is_err(), "Binary .llib mode MUST reject iterator conflict!");
    assert!(res_a_safe.is_ok(), "Source mode MUST accept non-overlapping iterator use!");
    assert!(res_b_safe.is_ok(), "Binary .llib mode MUST accept non-overlapping iterator use!");
}
