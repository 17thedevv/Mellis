use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p2b_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<luna_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = check(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

// -----------------------------------------------------------------------------
// Test Matrix for P2-B: Raw Pointer Safety & Unsafe Boundary (20 Tests)
// -----------------------------------------------------------------------------

// 01. Dereference of safe reference (&i32 or &rw i32) is allowed outside unsafe
#[test]
fn test_case_01_safe_reference_deref_allowed_outside_unsafe() {
    let src = r#"
        fn foo(x: &i32) -> i32 {
            *x
        }
        fn bar(x: &rw i32) -> i32 {
            *x
        }
        fn main() -> i32 {
            dec val = 42;
            dec rw val_rw = 100;
            foo(&val) + bar(&rw val_rw)
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(success, "Safe reference dereference outside unsafe should compile cleanly: {:?}", diags);
}

// 02. Dereference of raw pointer (*p) outside unsafe is rejected with E_UNSAFE_DEREF_OUTSIDE_UNSAFE
#[test]
fn test_case_02_raw_pointer_deref_outside_unsafe_rejected() {
    let src = r#"
        fn deref_raw(p: *i32) -> i32 {
            *p
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_02", src);
    assert!(!success, "Raw pointer dereference outside unsafe should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_UNSAFE_DEREF_OUTSIDE_UNSAFE"));
    assert!(err.is_some(), "Expected E_UNSAFE_DEREF_OUTSIDE_UNSAFE error, got: {:?}", diags);
}

// 03. Dereference of raw pointer (*p) inside unsafe block is allowed
#[test]
fn test_case_03_raw_pointer_deref_inside_unsafe_block_allowed() {
    let src = r#"
        fn read_raw(p: *i32) -> i32 {
            unsafe {
                return *p;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_03", src);
    assert!(success, "Raw pointer dereference inside unsafe block should succeed: {:?}", diags);
}

// 04. Writing through mutable raw pointer (*p = val) inside unsafe block is allowed
#[test]
fn test_case_04_raw_pointer_write_inside_unsafe_block_allowed() {
    let src = r#"
        fn write_raw(p: *rw i32, val: i32) {
            unsafe {
                *p = val;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_04", src);
    assert!(success, "Writing through mutable raw pointer inside unsafe should succeed: {:?}", diags);
}

// 05. Writing through immutable raw pointer (*p = val where p: *T) is rejected with E_CANNOT_MUTATE_IMMUTABLE_POINTER
#[test]
fn test_case_05_immutable_raw_pointer_write_rejected() {
    let src = r#"
        fn write_imm(p: *i32, val: i32) {
            unsafe {
                *p = val;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(!success, "Writing through immutable raw pointer should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_CANNOT_MUTATE_IMMUTABLE_POINTER"));
    assert!(err.is_some(), "Expected E_CANNOT_MUTATE_IMMUTABLE_POINTER error, got: {:?}", diags);
}

// 06. Pointer arithmetic (p + 1) outside unsafe block is rejected with E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE
#[test]
fn test_case_06_pointer_arithmetic_outside_unsafe_rejected() {
    let src = r#"
        fn offset_ptr(p: *i32) -> *i32 {
            p + 1
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_06", src);
    assert!(!success, "Pointer arithmetic outside unsafe should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE"));
    assert!(err.is_some(), "Expected E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE error, got: {:?}", diags);
}

// 07. Pointer arithmetic (p + 1) inside unsafe block is allowed
#[test]
fn test_case_07_pointer_arithmetic_inside_unsafe_allowed() {
    let src = r#"
        fn offset_ptr(p: *i32) -> *i32 {
            unsafe {
                return p + 1;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_07", src);
    assert!(success, "Pointer arithmetic inside unsafe block should succeed: {:?}", diags);
}

// 08. Calling unsafe fn outside unsafe block is rejected with E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE
#[test]
fn test_case_08_call_unsafe_fn_outside_unsafe_rejected() {
    let src = r#"
        unsafe fn danger() -> i32 {
            return 42;
        }
        fn main() -> i32 {
            danger()
        }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(!success, "Calling unsafe fn outside unsafe should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE"));
    assert!(err.is_some(), "Expected E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE error, got: {:?}", diags);
}

// 09. Calling unsafe fn inside unsafe block is allowed
#[test]
fn test_case_09_call_unsafe_fn_inside_unsafe_allowed() {
    let src = r#"
        unsafe fn danger() -> i32 {
            return 42;
        }
        fn main() -> i32 {
            unsafe {
                return danger();
            }
        }
    "#;
    let (success, diags) = run_compiler("test_09", src);
    assert!(success, "Calling unsafe fn inside unsafe block should succeed: {:?}", diags);
}

// 10. Unsafe fn body automatically establishes an unsafe context
#[test]
fn test_case_10_unsafe_fn_body_is_unsafe_context() {
    let src = r#"
        unsafe fn read_offset(p: *i32) -> i32 {
            dec next = p + 1;
            return *next;
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(success, "Unsafe operations directly inside unsafe fn body should succeed: {:?}", diags);
}

// 11. Safe borrowck rules remain active inside unsafe block (Invariant 2)
#[test]
fn test_case_11_borrowck_still_active_inside_unsafe() {
    let src = r#"
        fn main() -> i32 {
            dec rw x = 10;
            unsafe {
                dec r1 = &rw x;
                dec r2 = &rw x;
                *r1 = 20;
                *r2 = 30;
            }
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_11", src);
    assert!(!success, "Borrowck conflicts inside unsafe block must still be rejected");
    let borrow_err = diags.iter().find(|d| d.message.contains("already borrowed as &rw") || d.message.contains("Cannot borrow"));
    assert!(borrow_err.is_some(), "Expected borrowck error inside unsafe block, got: {:?}", diags);
}

// 12. Move analysis remains active inside unsafe block (Invariant 2)
#[test]
fn test_case_12_move_analysis_still_active_inside_unsafe() {
    let src = r#"
        struct NonCopy {
            val: i32,
        }

        fn main() -> i32 {
            dec a = NonCopy { val: 10 };
            unsafe {
                dec b = a;
                dec c = a;
                return c.val;
            }
        }
    "#;
    let (success, diags) = run_compiler("test_12", src);
    assert!(!success, "Use of moved value inside unsafe block must still be rejected");
    let move_err = diags.iter().find(|d| d.message.contains("E_USE_OF_MOVED_VALUE") || d.message.to_lowercase().contains("use of moved value"));
    assert!(move_err.is_some(), "Expected use of moved value error inside unsafe block, got: {:?}", diags);
}

// 13. Casting safe reference to raw pointer (&x as *i32) is safe and allowed outside unsafe
#[test]
fn test_case_13_reference_to_raw_cast_is_safe() {
    let src = r#"
        fn get_raw(x: &i32) -> *i32 {
            x as *i32
        }
        fn get_raw_mut(x: &rw i32) -> *rw i32 {
            x as *rw i32
        }
        fn main() -> i32 {
            dec rw val = 100;
            dec p1 = get_raw(&val);
            dec p2 = get_raw_mut(&rw val);
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_13", src);
    assert!(success, "Casting reference to raw pointer is safe and should compile: {:?}", diags);
}

// 14. Dereferencing null raw pointer in comptime is rejected with E_NULL_POINTER_DEREFERENCE
#[test]
fn test_case_14_comptime_null_pointer_deref_rejected() {
    let src = r#"
        const BAD: i32 = comptime {
            dec p: *i32 = 0 as *i32;
            unsafe {
                *p
            }
        };

        fn main() -> i32 {
            BAD
        }
    "#;
    let (success, diags) = run_compiler("test_14", src);
    assert!(!success, "Dereferencing null pointer in comptime must be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_NULL_POINTER_DEREFERENCE") || d.message.contains("null pointer"));
    assert!(err.is_some(), "Expected E_NULL_POINTER_DEREFERENCE error, got: {:?}", diags);
}

// 15. Comptime use-after-free is trapped deterministically with E_USE_AFTER_FREE
#[test]
fn test_case_15_comptime_use_after_free_rejected() {
    use luna_mvir::interp::MvirInterpreter;
    use luna_mvir::mvir::*;
    use luna_semantic::{SemanticContext, ComptimeError};

    let ctx = SemanticContext::new();
    let mut func = Function {
        name: GlobalId { name: "test_uaf".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: luna_semantic::SemanticTypeId(0),
        blocks: Vec::new(),
        values: Vec::new(),
    };
    
    // v0 = HeapAlloc
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::HeapAlloc,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v1 = Drop { value: v0, ty, callee: None }
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::Drop {
            value: Operand::Value(ValueId(0)),
            ty: luna_semantic::SemanticTypeId(0),
            callee: None,
        },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v2 = Load { ptr: v0 } -> Use after free!
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) },
        origin: ValueOrigin::Temporary,
        span: None,
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(2))) }),
    });

    let module = Module {
        functions: vec![func.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&func, vec![]);
    assert!(res.is_err(), "Expected evaluation error for use after free");
    match res.unwrap_err() {
        ComptimeError::UseAfterFree => {
            // Success: trapped deterministically with UseAfterFree
        }
        other => panic!("Expected ComptimeError::UseAfterFree, got: {:?}", other),
    }
}

// 16. Synthesizing safe reference from raw pointer (&rw (*p)) inside unsafe is allowed
#[test]
fn test_case_16_raw_derived_reference_local_borrowck() {
    let src = r#"
        fn derive_ref(p: *rw i32) -> i32 {
            unsafe {
                dec r: &rw i32 = &rw (*p);
                *r = 42;
                return *r;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_16", src);
    assert!(success, "Synthesizing safe reference from raw pointer inside unsafe should succeed: {:?}", diags);
}

// 17. Calling indirect function pointer of type `unsafe fn` outside unsafe is rejected with E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE
#[test]
fn test_case_17_call_unsafe_fn_indirect_pointer_outside_unsafe_rejected() {
    let src = r#"
        fn call_indirect(f: unsafe fn() -> i32) -> i32 {
            f()
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_17", src);
    assert!(!success, "Calling indirect unsafe fn pointer outside unsafe should be rejected");
    let err = diags.iter().find(|d| d.message.contains("E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE"));
    assert!(err.is_some(), "Expected E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE error, got: {:?}", diags);
}

// 18. Safe copying and passing of raw pointers across function boundaries without unsafe (Model B)
#[test]
fn test_case_18_safe_copy_and_passing_of_raw_pointer() {
    let src = r#"
        fn pass_ptr(p: *i32) -> *i32 {
            dec copy = p;
            return copy;
        }
        fn main() -> i32 {
            dec val = 10;
            dec ptr: *i32 = &val as *i32;
            dec res = pass_ptr(ptr);
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_18", src);
    assert!(success, "Copying and passing raw pointers is safe under Model B: {:?}", diags);
}

// 19. Raw pointer aliasing bypass: two mutable pointers writing to memory without borrowck conflict
#[test]
fn test_case_19_raw_pointer_aliasing_two_writes_allowed() {
    let src = r#"
        fn write_aliased(p: *rw i32, q: *rw i32) {
            unsafe {
                *p = 10;
                *q = 20;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_19", src);
    assert!(success, "Multiple mutable raw pointers writing in unsafe should not conflict: {:?}", diags);
}

// 20. Pointer arithmetic element stride in execution and compilation
#[test]
fn test_case_20_pointer_arithmetic_element_stride() {
    use luna_mvir::interp::{MvirInterpreter, RuntimeValue, Address};
    use luna_mvir::mvir::*;
    use luna_semantic::SemanticContext;

    // 20a. Verify compiler accepts pointer arithmetic with element stride
    let src = r#"
        fn ptr_math(p: *i32) -> *i32 {
            unsafe {
                dec p2 = p + 2;
                dec p3 = p2 - 1;
                return p3;
            }
        }
        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_20", src);
    assert!(success, "Pointer arithmetic with offsets in unsafe should compile: {:?}", diags);

    // 20b. Verify interpreter evaluates element stride offset
    let ctx = SemanticContext::new();
    let mut func = Function {
        name: GlobalId { name: "test_stride".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: luna_semantic::SemanticTypeId(0),
        blocks: Vec::new(),
        values: Vec::new(),
    };

    // v0 = Alloca
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::Alloca,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v1 = Add { left: v0, right: 3 }
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::Add { left: Operand::Value(ValueId(0)), right: Operand::Number("3".to_string()) },
        origin: ValueOrigin::Temporary,
        span: None,
    });
    // v2 = Sub { left: v1, right: 1 }
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::Sub { left: Operand::Value(ValueId(1)), right: Operand::Number("1".to_string()) },
        origin: ValueOrigin::Temporary,
        span: None,
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(2))) }),
    });

    let module = Module {
        functions: vec![func.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let res = interp.eval_function(&func, vec![]).expect("Stride evaluation failed");
    match res {
        RuntimeValue::Pointer(Address::Stack { offset, .. }) => {
            assert_eq!(offset, 2, "Expected offset to be 0 + 3 - 1 = 2 elements");
        }
        other => panic!("Expected stack pointer with offset, got: {:?}", other),
    }
}
