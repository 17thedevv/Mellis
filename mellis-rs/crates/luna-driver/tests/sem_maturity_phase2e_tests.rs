use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};
use std::fs;

fn test_check(name: &str, src: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let options = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    check(name, src.to_string(), &options)
}

// ============================================================================
// Phase 2E-A: Extern-Call Semantics & Source Conformance
// ============================================================================

// EXTERN-01: Plain primitive/value argument -> no borrow effect on caller places
#[test]
fn sem_extern_01_val_args_no_borrow_effects() {
    let src = r#"
        extern fn ext_add(a: i32, b: i32) -> i32;

        fn main() -> i32 {
            dec rw x = 10;
            dec res = ext_add(x, 20);
            x = x + 1;
            return x + res;
        }
    "#;
    let res = test_check("sem_extern_01", src);
    assert!(res.is_ok(), "Primitive arguments passed by value must not lock caller places: {:?}", res.err());
}

// EXTERN-02: Reference-free user-defined aggregate -> no provenance effect
#[test]
fn sem_extern_02_ref_free_aggregate_by_value() {
    let src = r#"
        #[repr(C)]
        struct Point {
            x: i32,
            y: i32,
        }

        extern fn ext_consume_point(p: Point);

        fn main() -> i32 {
            dec rw a = 10;
            dec p = Point { x: a, y: 20 };
            ext_consume_point(p);
            a = 42;
            return a;
        }
    "#;
    let res = test_check("sem_extern_02", src);
    assert!(res.is_ok(), "Reference-free aggregate passed by value must not create borrow effects: {:?}", res.err());
}

// EXTERN-03: RULE-EXTERN-AGGREGATE-01 - By-Value Aggregates at FFI Boundary
#[test]
fn sem_extern_03_aggregate_carrying_ref() {
    // Control 03A: By-value struct parameter containing &T -> rejected with E2030
    let src_shared = r#"
        #[repr(C)]
        struct SharedHolder {
            p: &i32,
        }

        extern fn ext_consume_shared(h: SharedHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_shared = test_check("sem_extern_03_shared", src_shared);
    assert!(res_shared.is_err(), "By-value aggregate containing &T must be rejected");
    let errs_shared = res_shared.err().unwrap();
    assert!(
        errs_shared.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_shared
    );

    // Control 03B: By-value struct parameter containing &rw T -> rejected with E2030
    let src_mut = r#"
        #[repr(C)]
        struct MutHolder {
            p: &rw i32,
        }

        extern fn ext_consume_mut(h: MutHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_mut = test_check("sem_extern_03_mut", src_mut);
    assert!(res_mut.is_err(), "By-value aggregate containing &rw T must be rejected");
    let errs_mut = res_mut.err().unwrap();
    assert!(
        errs_mut.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_mut
    );

    // Control 03C: Nested struct containing safe reference -> rejected with E2030
    let src_nested = r#"
        #[repr(C)]
        struct Inner {
            r: &i32,
        }

        #[repr(C)]
        struct Outer {
            inner: Inner,
            val: i32,
        }

        extern fn ext_consume_nested(o: Outer);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_nested = test_check("sem_extern_03_nested", src_nested);
    assert!(res_nested.is_err(), "By-value aggregate transitively containing safe reference must be rejected");
    let errs_nested = res_nested.err().unwrap();
    assert!(
        errs_nested.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_nested
    );

    // Control 03D: Type alias wrapping safe reference inside aggregate -> rejected with E2030
    let src_alias = r#"
        type RefAlias = &i32;

        #[repr(C)]
        struct AliasHolder {
            r: RefAlias,
        }

        extern fn ext_consume_alias(a: AliasHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_alias = test_check("sem_extern_03_alias", src_alias);
    assert!(res_alias.is_err(), "By-value aggregate containing aliased safe reference must be rejected");
    let errs_alias = res_alias.err().unwrap();
    assert!(
        errs_alias.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_alias
    );

    // Control 03E: Reference-free repr(C) aggregate -> valid
    let src_ref_free = r#"
        #[repr(C)]
        struct CleanPoint {
            x: i32,
            y: i32,
        }

        extern fn ext_consume_clean(p: CleanPoint);

        fn main() -> i32 {
            dec p = CleanPoint { x: 1, y: 2 };
            ext_consume_clean(p);
            return 0;
        }
    "#;
    let res_ref_free = test_check("sem_extern_03_ref_free", src_ref_free);
    assert!(res_ref_free.is_ok(), "Reference-free repr(C) aggregate must succeed: {:?}", res_ref_free.err());

    // Control 03F: Aggregate containing safe ref returned from extern -> rejected with E2030
    let src_return_agg = r#"
        #[repr(C)]
        struct RetWrap {
            r: &i32,
        }

        extern fn ext_produce_wrap() -> RetWrap;

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_return_agg = test_check("sem_extern_03_return_agg", src_return_agg);
    assert!(res_return_agg.is_err(), "Extern function returning aggregate containing safe reference must be rejected");
    let errs_ret = res_return_agg.err().unwrap();
    assert!(
        errs_ret.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && (d.message.contains("safe reference") || d.message.contains("pointer capability")))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_ret
    );

    // Control 03G: Struct parameter containing raw pointer *i32 -> rejected with E2030
    let src_raw_ptr = r#"
        #[repr(C)]
        struct RawHolder {
            p: *i32,
        }

        extern fn ext_consume_raw(h: RawHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_raw_ptr = test_check("sem_extern_03_raw_ptr", src_raw_ptr);
    assert!(res_raw_ptr.is_err(), "By-value aggregate containing *i32 must be rejected");
    let errs_raw = res_raw_ptr.err().unwrap();
    assert!(
        errs_raw.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("pointer capability"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_raw
    );

    // Control 03H: Struct parameter containing raw mut pointer *rw i32 -> rejected with E2030
    let src_raw_mut = r#"
        #[repr(C)]
        struct RawMutHolder {
            p: *rw i32,
        }

        extern fn ext_consume_raw_mut(h: RawMutHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_raw_mut = test_check("sem_extern_03_raw_mut", src_raw_mut);
    assert!(res_raw_mut.is_err(), "By-value aggregate containing *rw i32 must be rejected");
    let errs_raw_mut = res_raw_mut.err().unwrap();
    assert!(
        errs_raw_mut.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("pointer capability"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_raw_mut
    );

    // Control 03I: Nested struct containing raw pointer aggregate -> rejected with E2030
    let src_nested_raw = r#"
        #[repr(C)]
        struct InnerRaw {
            p: *i32,
        }

        #[repr(C)]
        struct OuterRaw {
            inner: InnerRaw,
            val: i32,
        }

        extern fn ext_consume_nested_raw(o: OuterRaw);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_nested_raw = test_check("sem_extern_03_nested_raw", src_nested_raw);
    assert!(res_nested_raw.is_err(), "By-value aggregate transitively containing *i32 must be rejected");
    let errs_nested_raw = res_nested_raw.err().unwrap();
    assert!(
        errs_nested_raw.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("pointer capability"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_nested_raw
    );

    // Control 03J: Type alias wrapping raw pointer aggregate -> rejected with E2030
    let src_alias_raw = r#"
        type RawAlias = *i32;

        #[repr(C)]
        struct AliasRawHolder {
            p: RawAlias,
        }

        extern fn ext_consume_alias_raw(a: AliasRawHolder);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_alias_raw = test_check("sem_extern_03_alias_raw", src_alias_raw);
    assert!(res_alias_raw.is_err(), "By-value aggregate containing aliased raw pointer must be rejected");
    let errs_alias_raw = res_alias_raw.err().unwrap();
    assert!(
        errs_alias_raw.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("pointer capability"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_alias_raw
    );

    // Control 03K: Direct raw pointers parameter (*i32 and *rw i32) remain valid
    let src_direct_raw = r#"
        extern fn ext_direct_raw(p1: *i32, p2: *rw i32);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_direct_raw = test_check("sem_extern_03_direct_raw", src_direct_raw);
    assert!(res_direct_raw.is_ok(), "Direct raw pointer parameters must remain valid: {:?}", res_direct_raw.err());

    // Control 03L: Extern function returning aggregate containing raw pointer -> rejected with E2030
    let src_ret_raw_agg = r#"
        #[repr(C)]
        struct RawRetWrap {
            p: *i32,
        }

        extern fn ext_produce_raw_wrap() -> RawRetWrap;

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_ret_raw_agg = test_check("sem_extern_03_ret_raw_agg", src_ret_raw_agg);
    assert!(res_ret_raw_agg.is_err(), "Extern function returning aggregate containing raw pointer must be rejected");
    let errs_ret_raw = res_ret_raw_agg.err().unwrap();
    assert!(
        errs_ret_raw.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("pointer capability"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_ret_raw
    );

    // Control 03M: Function pointer aggregate with primitive parameter -> Valid (non-provenance-bearing code pointer)
    let src_callback_only = r#"
        #[repr(C)]
        struct CallbackOnly {
            cb: fn(i32) -> i32,
        }

        extern fn ext_register_callback(c: CallbackOnly);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_cb_only = test_check("sem_extern_03_cb_only", src_callback_only);
    assert!(res_cb_only.is_ok(), "Function pointer aggregate CallbackOnly must succeed: {:?}", res_cb_only.err());

    // Control 03N: Function pointer aggregate with raw-pointer parameter -> Valid (callable signature, not data provenance)
    let src_callback_ptr = r#"
        #[repr(C)]
        struct CallbackPtr {
            cb: fn(*i32) -> i32,
        }

        extern fn ext_register_callback_ptr(c: CallbackPtr);

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_cb_ptr = test_check("sem_extern_03_cb_ptr", src_callback_ptr);
    assert!(res_cb_ptr.is_ok(), "Function pointer aggregate CallbackPtr must succeed: {:?}", res_cb_ptr.err());

    // Control 03-PARITY: Provider .ln declaring safe-reference aggregate FFI compiled to .llib must be rejected
    let sysroot = Sysroot::discover_for_test().expect("discover test sysroot").root().to_string_lossy().to_string();
    let temp_dir = std::env::temp_dir().join(format!("luna_ext03_parity_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let prov_src = r#"
        module bad_prov {
            #[repr(C)]
            export struct BadHolder {
                p: &rw i32,
            }
            export extern fn ext_bad(h: BadHolder);
        }
    "#;
    let prov_ln = temp_dir.join("bad_prov.ln");
    let prov_llib = temp_dir.join("bad_prov.llib");
    fs::write(&prov_ln, prov_src).expect("write bad_prov.ln");
    let compile_opts = CompilerOptions {
        output_path: Some(prov_llib.to_string_lossy().to_string()),
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let compile_res = compile(prov_ln.to_str().unwrap(), prov_src.to_string(), &compile_opts);
    assert!(compile_res.is_err(), "Compiling provider with by-value safe-ref aggregate to .llib must be rejected");
    let compile_errs = compile_res.err().unwrap();
    assert!(
        compile_errs.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030 on provider compilation, got: {:?}",
        compile_errs
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

// EXTERN-04: &T synchronous call behavior -> active during call, accessible after
#[test]
fn sem_extern_04_shared_ref_synchronous_borrow() {
    let src = r#"
        extern fn ext_read(p: &i32);

        fn main() -> i32 {
            dec rw x = 10;
            ext_read(&x);
            x = 20;
            return x;
        }
    "#;
    let res = test_check("sem_extern_04", src);
    assert!(res.is_ok(), "Safe &T passed to extern must be synchronous borrow, releasing place after call: {:?}", res.err());
}

// EXTERN-05: &rw T synchronous call behavior -> exclusive during call, accessible after
#[test]
fn sem_extern_05_mut_ref_synchronous_exclusive_borrow() {
    let src = r#"
        extern fn ext_mutate(p: &rw i32);

        fn main() -> i32 {
            dec rw x = 10;
            ext_mutate(&rw x);
            x = x + 1;
            return x;
        }
    "#;
    let res = test_check("sem_extern_05", src);
    assert!(res.is_ok(), "Safe &rw T passed to extern must be exclusive synchronous borrow: {:?}", res.err());
}

// EXTERN-06: Two genuinely incompatible reference arguments to extern call -> E3003
#[test]
fn sem_extern_06_conflicting_refs_to_extern_call_rejected() {
    let src = r#"
        extern fn ext_two_refs(a: &rw i32, b: &rw i32);

        fn main() -> i32 {
            dec rw x = 10;
            ext_two_refs(&rw x, &rw x);
            return x;
        }
    "#;
    let res = test_check("sem_extern_06", src);
    assert!(res.is_err(), "Conflicting mutable references to same extern call must be rejected");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("Cannot borrow") && d.message.contains("already borrowed as &rw")),
        "Expected borrow conflict E3003, got: {:?}",
        errs
    );
}

// EXTERN-07: Raw pointer derived from tracked borrow without #[sync_noescape] -> may-escape
#[test]
fn sem_extern_07_raw_ptr_derived_from_borrow_may_escape() {
    let src = r#"
        extern fn ext_ptr(p: *i32);

        fn main() -> i32 {
            dec rw x = 10;
            ext_ptr(&x as *i32);
            x = 20;
            return x;
        }
    "#;
    let res = test_check("sem_extern_07", src);
    assert!(res.is_err(), "Raw pointer without #[sync_noescape] derived from local loan must retain loan into escaped_loans");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("Cannot write to") && d.message.contains("borrowed as &")),
        "Expected write-borrow conflict on escaped loan, got: {:?}",
        errs
    );
}

// EXTERN-08: Raw pointer + existing #[sync_noescape] contract -> loan ends after call
#[test]
fn sem_extern_08_raw_ptr_with_sync_noescape_allows_mutation_after() {
    let src = r#"
        extern fn ext_sync_ptr(#[sync_noescape] p: *i32);

        fn main() -> i32 {
            dec rw x = 10;
            ext_sync_ptr(&x as *i32);
            x = 20;
            return x;
        }
    "#;
    let res = test_check("sem_extern_08", src);
    assert!(res.is_ok(), "Raw pointer with #[sync_noescape] must release loan after call: {:?}", res.err());
}

// EXTERN-09: Opaque raw pointer without tracked provenance -> do not fabricate a loan
#[test]
fn sem_extern_09_opaque_raw_ptr_without_provenance_no_loan() {
    let src = r#"
        extern fn ext_ptr(p: *i32);

        fn test_fn(raw: *i32) -> i32 {
            ext_ptr(raw);
            return 0;
        }
    "#;
    let res = test_check("sem_extern_09", src);
    assert!(res.is_ok(), "Opaque raw pointer without tracked provenance must not fabricate a loan: {:?}", res.err());
}

// EXTERN-10: Extern return plain value
#[test]
fn sem_extern_10_extern_return_value() {
    let src = r#"
        extern fn ext_get_val() -> i32;

        fn main() -> i32 {
            dec v = ext_get_val();
            return v + 1;
        }
    "#;
    let res = test_check("sem_extern_10", src);
    assert!(res.is_ok(), "Extern function returning plain value must succeed: {:?}", res.err());
}

// EXTERN-11: Extern return raw pointer (allocator pattern)
#[test]
fn sem_extern_11_extern_return_raw_ptr() {
    let src = r#"
        extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        fn main() -> i32 {
            dec p = __mellis_alloc(8 as u64, 8 as u64);
            __mellis_dealloc(p, 8 as u64, 8 as u64);
            return 0;
        }
    "#;
    let res = test_check("sem_extern_11", src);
    assert!(res.is_ok(), "Extern returning raw pointer without provenance must succeed: {:?}", res.err());
}

// EXTERN-12: Safe reference in loop -> synchronous release per iteration
#[test]
fn sem_extern_12_extern_call_in_loop_safe_ref() {
    let src = r#"
        extern fn ext_mutate(p: &rw i32);

        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                ext_mutate(&rw x);
                x = x + 1;
                i = i + 1;
            }
            return x;
        }
    "#;
    let res = test_check("sem_extern_12", src);
    assert!(res.is_ok(), "Synchronous safe &rw reference in loop must be released per iteration: {:?}", res.err());
}

// EXTERN-13: Raw pointer derived from loan without #[sync_noescape] escapes -> subsequent write in loop rejected
#[test]
fn sem_extern_13_extern_raw_ptr_may_escape_in_loop() {
    let src = r#"
        extern fn ext_retain_ptr(p: *i32);

        fn main() -> i32 {
            dec rw x = 10;
            dec rw i = 0;
            while i < 3 {
                ext_retain_ptr(&x as *i32);
                x = x + 1;
                i = i + 1;
            }
            return 0;
        }
    "#;
    let res = test_check("sem_extern_13", src);
    assert!(res.is_err(), "Raw pointer without #[sync_noescape] must retain loan in escaped_loans and reject subsequent write");
    let errs = res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("Cannot write to") && d.message.contains("borrowed as &")),
        "Expected write-borrow conflict on escaped loan, got: {:?}",
        errs
    );
}

// EXTERN-14: RULE-EXTERN-REF-RETURN-01 - Safe-Reference Returns
#[test]
fn sem_extern_14_extern_return_reference() {
    // Control 14A: extern returning &T without contract -> rejected with E2030
    let src_shared = r#"
        extern fn ext_get_shared() -> &i32;

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_shared = test_check("sem_extern_14_shared", src_shared);
    assert!(res_shared.is_err(), "Extern function returning &T without lifetime contract must be rejected");
    let errs_shared = res_shared.err().unwrap();
    assert!(
        errs_shared.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_shared
    );

    // Control 14B: extern returning &rw T without contract -> rejected with E2030
    let src_mut = r#"
        extern fn ext_get_mut() -> &rw i32;

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res_mut = test_check("sem_extern_14_mut", src_mut);
    assert!(res_mut.is_err(), "Extern function returning &rw T without lifetime contract must be rejected");
    let errs_mut = res_mut.err().unwrap();
    assert!(
        errs_mut.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030, got: {:?}",
        errs_mut
    );

    // Control 14C: extern returning *T (const raw pointer) -> valid
    let src_raw_const = r#"
        extern fn ext_get_raw_const() -> *i32;

        fn main() -> i32 {
            dec p = ext_get_raw_const();
            return 0;
        }
    "#;
    let res_raw_const = test_check("sem_extern_14_raw_const", src_raw_const);
    assert!(res_raw_const.is_ok(), "Extern returning *T must succeed: {:?}", res_raw_const.err());

    // Control 14D: extern returning *rw T (mut raw pointer) -> valid
    let src_raw_mut = r#"
        extern fn ext_get_raw_mut() -> *rw u8;

        fn main() -> i32 {
            dec p = ext_get_raw_mut();
            return 0;
        }
    "#;
    let res_raw_mut = test_check("sem_extern_14_raw_mut", src_raw_mut);
    assert!(res_raw_mut.is_ok(), "Extern returning *rw T must succeed: {:?}", res_raw_mut.err());

    // Control 14-PARITY: Provider .ln declaring safe-reference return compiled to .llib must be rejected
    let sysroot = Sysroot::discover_for_test().expect("discover test sysroot").root().to_string_lossy().to_string();
    let temp_dir = std::env::temp_dir().join(format!("luna_ext14_parity_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let prov_src = r#"
        module bad_ret_prov {
            export extern fn ext_bad_ret() -> &i32;
        }
    "#;
    let prov_ln = temp_dir.join("bad_ret_prov.ln");
    let prov_llib = temp_dir.join("bad_ret_prov.llib");
    fs::write(&prov_ln, prov_src).expect("write bad_ret_prov.ln");
    let compile_opts = CompilerOptions {
        output_path: Some(prov_llib.to_string_lossy().to_string()),
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let compile_res = compile(prov_ln.to_str().unwrap(), prov_src.to_string(), &compile_opts);
    assert!(compile_res.is_err(), "Compiling provider with safe-ref return to .llib must be rejected");
    let compile_errs = compile_res.err().unwrap();
    assert!(
        compile_errs.iter().any(|d| d.message.contains("E2030") || (d.message.contains("FFI-safe") && d.message.contains("safe reference"))),
        "Expected NonFfiSafeType E2030 on provider compilation, got: {:?}",
        compile_errs
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// Phase 2E-B: Artifact Parity (SEM-GAP-20)
// ============================================================================

/// EXTERN-PARITY-01: SEM-GAP-20 Resolution & Source/.llib Parity
///
/// #[sync_noescape] semantic metadata is preserved across .llib serialization
/// and deserialization, guaranteeing exact semantic equivalence between source
/// and binary provider artifacts.
#[test]
fn sem_extern_parity_01_sync_noescape_llib_parity() {
    let sysroot = Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string();

    let temp_dir = std::env::temp_dir().join(format!(
        "luna_sync_noescape_parity_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let prov_src = r#"
        module ffi_prov {
            export extern fn safe_read(#[sync_noescape] p: *i32);
            export extern fn escaping_read(p: *i32);
        }
    "#;

    let prov_ln = temp_dir.join("ffi_prov.ln");
    let prov_llib = temp_dir.join("ffi_prov.llib");
    fs::write(&prov_ln, prov_src).expect("write ffi_prov.ln");

    let check_opts = CompilerOptions {
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot.clone()],
        quiet: true,
        ..Default::default()
    };

    // -------------------------------------------------------------
    // Phase 1: Verify source mode (.ln)
    // -------------------------------------------------------------
    let consumer_safe = r#"
        import "ffi_prov";

        fn main() -> i32 {
            dec rw x = 10;
            ffi_prov::safe_read(&x as *i32);
            x = 20;
            return x;
        }
    "#;
    let consumer_safe_path = temp_dir.join("consumer_safe.ln");
    fs::write(&consumer_safe_path, consumer_safe).expect("write consumer_safe.ln");

    let res_src_safe = check(consumer_safe_path.to_str().unwrap(), consumer_safe.to_string(), &check_opts);
    assert!(res_src_safe.is_ok(), "Source mode safe_read must succeed: {:?}", res_src_safe.err());

    let consumer_escaping = r#"
        import "ffi_prov";

        fn main() -> i32 {
            dec rw x = 10;
            ffi_prov::escaping_read(&x as *i32);
            x = 20;
            return x;
        }
    "#;
    let consumer_escaping_path = temp_dir.join("consumer_escaping.ln");
    fs::write(&consumer_escaping_path, consumer_escaping).expect("write consumer_escaping.ln");

    let res_src_escaping = check(consumer_escaping_path.to_str().unwrap(), consumer_escaping.to_string(), &check_opts);
    assert!(res_src_escaping.is_err(), "Source mode escaping_read must reject conflicting write");

    // -------------------------------------------------------------
    // Phase 2: Compile provider to .llib and remove .ln
    // -------------------------------------------------------------
    let compile_opts = CompilerOptions {
        output_path: Some(prov_llib.to_string_lossy().to_string()),
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot.clone()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let prov_res = compile(prov_ln.to_str().unwrap(), prov_src.to_string(), &compile_opts);
    assert!(prov_res.is_ok(), "Compiling ffi_prov to .llib must succeed: {:?}", prov_res.err());
    assert!(prov_llib.exists(), "ffi_prov.llib must exist");

    // Remove source provider to guarantee consumer resolves strictly from .llib
    fs::remove_file(&prov_ln).expect("remove ffi_prov.ln");

    // -------------------------------------------------------------
    // Phase 3: Verify artifact mode (.llib) parity
    // -------------------------------------------------------------
    let res_lib_safe = check(consumer_safe_path.to_str().unwrap(), consumer_safe.to_string(), &check_opts);
    assert!(
        res_lib_safe.is_ok(),
        "Artifact mode (.llib) safe_read must succeed (SEM-GAP-20 resolved): {:?}",
        res_lib_safe.err()
    );

    let res_lib_escaping = check(consumer_escaping_path.to_str().unwrap(), consumer_escaping.to_string(), &check_opts);
    assert!(
        res_lib_escaping.is_err(),
        "Artifact mode (.llib) escaping_read must reject conflicting write (negative control)"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

/// EXTERN-PARITY-02: Multi-Parameter Identity Control
///
/// Verifies that when an extern function accepts multiple raw pointers,
/// parameter indices are preserved exactly across the .llib boundary:
/// - Parameter 0: unannotated (*i32) -> MayEscape (locks referent)
/// - Parameter 1: annotated (#[sync_noescape] *i32) -> NoEscape (releases referent)
#[test]
fn sem_extern_parity_02_multi_param_sync_noescape_llib() {
    let sysroot = Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string();

    let temp_dir = std::env::temp_dir().join(format!(
        "luna_multi_param_parity_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let prov_src = r#"
        module multi_prov {
            export extern fn dual_ptr(a: *i32, #[sync_noescape] b: *i32);
        }
    "#;

    let prov_ln = temp_dir.join("multi_prov.ln");
    let prov_llib = temp_dir.join("multi_prov.llib");
    fs::write(&prov_ln, prov_src).expect("write multi_prov.ln");

    let compile_opts = CompilerOptions {
        output_path: Some(prov_llib.to_string_lossy().to_string()),
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot.clone()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let prov_res = compile(prov_ln.to_str().unwrap(), prov_src.to_string(), &compile_opts);
    assert!(prov_res.is_ok(), "Compiling multi_prov to .llib must succeed: {:?}", prov_res.err());
    assert!(prov_llib.exists(), "multi_prov.llib must exist");

    // Remove source provider to guarantee consumer resolves strictly from .llib
    fs::remove_file(&prov_ln).expect("remove multi_prov.ln");

    let check_opts = CompilerOptions {
        search_paths: vec![temp_dir.to_string_lossy().to_string(), sysroot.clone()],
        quiet: true,
        ..Default::default()
    };

    // Case 1: Mutating parameter 1 referent (b: #[sync_noescape]) -> MUST PASS
    let consumer_mutate_b = r#"
        import "multi_prov";

        fn main() -> i32 {
            dec rw x = 10;
            dec rw y = 20;
            multi_prov::dual_ptr(&x as *i32, &y as *i32);
            y = 200; // y was passed to param 1 (sync_noescape) -> allowed
            return y;
        }
    "#;
    let path_b = temp_dir.join("consumer_b.ln");
    fs::write(&path_b, consumer_mutate_b).expect("write consumer_b.ln");
    let res_b = check(path_b.to_str().unwrap(), consumer_mutate_b.to_string(), &check_opts);
    assert!(
        res_b.is_ok(),
        "Mutating parameter 1 referent (sync_noescape) must succeed: {:?}",
        res_b.err()
    );

    // Case 2: Mutating parameter 0 referent (a: unannotated) -> MUST FAIL (MayEscape)
    let consumer_mutate_a = r#"
        import "multi_prov";

        fn main() -> i32 {
            dec rw x = 10;
            dec rw y = 20;
            multi_prov::dual_ptr(&x as *i32, &y as *i32);
            x = 100; // x was passed to param 0 (unannotated) -> rejected
            return x;
        }
    "#;
    let path_a = temp_dir.join("consumer_a.ln");
    fs::write(&path_a, consumer_mutate_a).expect("write consumer_a.ln");
    let res_a = check(path_a.to_str().unwrap(), consumer_mutate_a.to_string(), &check_opts);
    assert!(
        res_a.is_err(),
        "Mutating parameter 0 referent (unannotated) must be rejected"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
