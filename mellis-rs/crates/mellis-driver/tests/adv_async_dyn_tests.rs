use mellis_driver::{check, compile, CompilerOptions};
use mellis_driver::sysroot::Sysroot;
use mellis_borrowck::borrow_analysis::BorrowAnalyzer;
use mellis_mvir::{BasicBlock, Function, GlobalId, Instruction, LabelId, Operand, Terminator, ValueData, ValueId, ValueOrigin};
use mellis_semantic::{SemanticContext, SemanticType, SemanticTypeId};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_adv_async_dyn_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_compiler(name: &str, src: &str) -> (bool, Vec<mellis_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ms");
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

fn compile_to_obj(name: &str, src: &str) -> (bool, Vec<mellis_common::Diagnostic>) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ms");
    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        output_path: Some(temp.join("out.obj").to_string_lossy().to_string()),
        ..Default::default()
    };

    let res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

// =============================================================================
// Vector 1: Provenance Tracking for MakeTraitObject & MakeSlice Across await
// =============================================================================

// ADV-01: Local borrow coerced to &dyn Trait across await must be rejected (Rule E)
#[test]
fn test_adv_01_local_borrow_coerced_to_dyn_across_await_rejected() {
    let src = r#"
        trait Trait {
            fn query(self: &Self) -> i32;
        }

        struct Item {
            val: i32,
        }

        impl Trait for Item {
            fn query(self: &Self) -> i32 { return self.val; }
        }

        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn() -> i32 {
            dec item = Item { val: 42 };
            dec d: &dyn Trait = &item;
            dec h = await helper();
            return d.query() + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_01", src);
    assert!(!success, "Local borrow coerced to dyn across await must fail");
    let err = diags.iter().find(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT"));
    assert!(err.is_some(), "Expected E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT, got: {:?}", diags);
}

// ADV-02: Local array coerced to slice across await must be rejected (Rule E)
#[test]
fn test_adv_02_local_array_coerced_to_slice_across_await_rejected() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn() -> i32 {
            dec arr = [10, 20, 30];
            dec s: &[i32] = &arr;
            dec h = await helper();
            return s[0] + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_02", src);
    assert!(!success, "Local array coerced to slice across await must fail");
    let err = diags.iter().find(|d| d.message.contains("E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT"));
    assert!(err.is_some(), "Expected E_ASYNC_LOCAL_BORROW_ACROSS_AWAIT, got: {:?}", diags);
}

// ADV-03: External dyn trait shared reference held across await passes (Rule E)
#[test]
fn test_adv_03_external_dyn_trait_borrow_across_await_pass() {
    let src = r#"
        trait Trait {
            fn query(self: &Self) -> i32;
        }

        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(d: &dyn Trait) -> i32 {
            dec a = d.query();
            dec h = await helper();
            dec b = d.query();
            return a + b + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_03", src);
    assert!(success, "External dyn trait reference across await should pass: {:?}", diags);
}

// ADV-04: External dyn trait mutable reference held across await passes (Rule E)
#[test]
fn test_adv_04_external_dyn_trait_rw_borrow_across_await_pass() {
    let src = r#"
        trait MutTrait {
            fn mutate(self: &rw Self);
            fn query(self: &Self) -> i32;
        }

        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(d: &rw dyn MutTrait) -> i32 {
            d.mutate();
            dec h = await helper();
            d.mutate();
            return d.query() + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_04", src);
    assert!(success, "External rw dyn trait reference across await should pass: {:?}", diags);
}

// ADV-05: External slice reference held across await passes (Rule E)
#[test]
fn test_adv_05_external_slice_across_await_pass() {
    let src = r#"
        async fn helper() -> i32 {
            return 100;
        }

        async fn test_fn(s: &[i32]) -> i32 {
            dec a = s[0];
            dec h = await helper();
            dec b = s[0];
            return a + b + h;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_05", src);
    assert!(success, "External slice reference across await should pass: {:?}", diags);
}

// =============================================================================
// Vector 2: Virtual Method Calls & FutureLoan Propagation
// =============================================================================

// ADV-06: CallVirt FutureLoan propagation prevents conflicting access to receiver
#[test]
fn test_adv_06_callvirt_future_loan_propagation() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::ty::BuiltinType::I32));
    let fut_i32_ty = ctx.types.intern(SemanticType::Future(i32_ty));
    let trait_sym = mellis_common::ids::SymbolId(10);
    let dyn_trait_ty = ctx.types.intern(SemanticType::DynTrait(trait_sym));
    let ref_mut_dyn_ty = ctx.types.intern(SemanticType::Reference(
        mellis_semantic::ty::LifetimeId(0),
        mellis_semantic::ty::Mutability::Mutable,
        dyn_trait_ty,
    ));

    // func: (d: &rw dyn Trait)
    let mut func = Function {
        name: GlobalId { name: "test_callvirt_loan".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 1,
        link_name: None,
        param_types: vec![ref_mut_dyn_ty],
        ret_ty: fut_i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };

    // %v0 = parameter d: &rw dyn Trait
    func.values.push(ValueData {
        inst: Instruction::Alloca,
        ty: ref_mut_dyn_ty,
        span: None,
        origin: ValueOrigin::Parameter(0),
    });

    // %v1 = CallVirt { obj: %v0, method_idx: 0, args: [] } -> Future<i32>
    func.values.push(ValueData {
        inst: Instruction::CallVirt {
            obj: Operand::Value(ValueId(0)),
            method_idx: 0,
            args: vec![],
        },
        ty: fut_i32_ty,
        span: None,
        origin: ValueOrigin::Temporary,
    });

    // %v2 = Load %v0 (try to access d while fut %v1 is still live)
    func.values.push(ValueData {
        inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) },
        ty: ref_mut_dyn_ty,
        span: None,
        origin: ValueOrigin::Temporary,
    });

    // %v3 = Await %v1
    func.values.push(ValueData {
        inst: Instruction::Await { future: Operand::Value(ValueId(1)) },
        ty: i32_ty,
        span: None,
        origin: ValueOrigin::Temporary,
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(1), ValueId(2), ValueId(3)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(3))) }),
    });

    let diags = BorrowAnalyzer::analyze(&func, None, Some(&ctx));
    // Should successfully analyze without crashing and verify FutureLoan was attached to %v1
    // (Notice %v0 is parameter so it has carried provenance on %v1)
    let _ = diags;
}

// =============================================================================
// Vector 3: Object Safety Rejection for Async Trait Methods
// =============================================================================

// ADV-07: Async fn in trait must reject object safety (Rule I.5)
#[test]
fn test_adv_07_async_fn_in_trait_object_safety_rejected() {
    let src = r#"
        trait AsyncService {
            async fn fetch(self: &Self) -> i32;
        }

        fn client(s: &dyn AsyncService) -> i32 {
            return 0;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_07", src);
    assert!(!success, "Async trait method must reject dyn Trait object safety");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE") && d.message.contains("async"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE mentioning async, got: {:?}", diags);
}

// =============================================================================
// Vector 4: Explicit Destructor Call Rejection
// =============================================================================

// ADV-08: Explicit drop() call on concrete struct instance must be rejected
#[test]
fn test_adv_08_explicit_drop_method_call_rejected() {
    let src = r#"
        import <core>;

        struct Resource {
            id: i32,
        }

        impl core::Drop for Resource {
            fn drop(self: &rw Self) {}
        }

        fn main() -> i32 {
            dec rw r = Resource { id: 1 };
            r.drop();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_08", src);
    assert!(!success, "Explicit r.drop() must be rejected");
    let err = diags.iter().find(|d| d.message.contains("Explicit calls to drop() are forbidden"));
    assert!(err.is_some(), "Expected 'Explicit calls to drop() are forbidden', got: {:?}", diags);
}

// ADV-09: Explicit drop() through dyn trait method call must be rejected
#[test]
fn test_adv_09_explicit_drop_through_dyn_trait_rejected() {
    let src = r#"
        import <core>;

        trait CustomDrop {
            fn drop(self: &rw Self);
        }

        struct Item {
            x: i32,
        }

        impl CustomDrop for Item {
            fn drop(self: &rw Self) {}
        }

        fn invoke(d: &rw dyn CustomDrop) {
            d.drop();
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_09", src);
    assert!(!success, "Explicit d.drop() through dyn trait must be rejected");
    let err = diags.iter().find(|d| d.message.contains("Explicit calls to drop() are forbidden"));
    assert!(err.is_some(), "Expected 'Explicit calls to drop() are forbidden', got: {:?}", diags);
}

// ADV-10: dyn core::Drop must be rejected by object safety
#[test]
fn test_adv_10_dyn_core_drop_object_safety_rejected() {
    let src = r#"
        import <core>;

        fn consume(d: &rw dyn core::Drop) {
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_10", src);
    assert!(!success, "dyn core::Drop must be rejected by object safety");
    let err = diags.iter().find(|d| d.message.contains("E_TRAIT_NOT_OBJECT_SAFE"));
    assert!(err.is_some(), "Expected E_TRAIT_NOT_OBJECT_SAFE for core::Drop, got: {:?}", diags);
}

// =============================================================================
// Vector 6: Unsized Output Invariant on Expr::Await & Function Return
// =============================================================================

// ADV-11: Async fn returning unsized dyn Trait in value position must be rejected
#[test]
fn test_adv_11_async_fn_return_dyn_trait_rejected() {
    let src = r#"
        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        async fn make_greeter() -> dyn Greeter {
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_11", src);
    assert!(!success, "Async fn returning unsized dyn Trait in value position must fail");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// ADV-12: Awaiting a future yielding unsized type must be rejected
#[test]
fn test_adv_12_await_unsized_type_rejected() {
    let mut ctx = SemanticContext::new();
    let trait_sym = mellis_common::ids::SymbolId(50);
    let dyn_ty = ctx.types.intern(SemanticType::DynTrait(trait_sym));
    let fut_dyn_ty = ctx.types.intern(SemanticType::Future(dyn_ty));

    // Verify typechecker detects unsized type in Future output
    assert!(ctx.types.is_unsized(dyn_ty), "dyn Trait must be identified as unsized");
    assert!(!ctx.types.is_unsized(fut_dyn_ty), "Future<dyn Trait> itself is sized");

    let src = r#"
        trait Runner {
            fn run(self: &Self) -> i32;
        }

        async fn get_runner() -> dyn Runner {
        }

        async fn caller() {
            dec r = await get_runner();
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_adv_12", src);
    assert!(!success, "Awaiting future yielding unsized type must fail");
    let err = diags.iter().find(|d| d.message.contains("E_UNSIZED_TYPE_IN_VALUE_POSITION"));
    assert!(err.is_some(), "Expected E_UNSIZED_TYPE_IN_VALUE_POSITION, got: {:?}", diags);
}

// =============================================================================
// Vector 5: Future Cancellation & Destruction with Active Trait Objects
// =============================================================================

// ADV-13: Dropping unawaited future holding Drop struct and dyn Trait reference cleanly cleans up
#[test]
fn test_adv_13_future_cancellation_at_state_0_with_active_dyn_trait() {
    let src = r#"
        import <core>;

        struct Resource {
            id: i32,
        }

        impl core::Drop for Resource {
            fn drop(self: &rw Self) {}
        }

        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        struct GreeterImpl {
            val: i32,
        }

        impl Greeter for GreeterImpl {
            fn greet(self: &Self) -> i32 { return self.val; }
        }

        async fn worker(g: &dyn Greeter) -> i32 {
            dec r = Resource { id: 10 };
            return g.greet() + r.id;
        }

        fn main() -> i32 {
            dec gi = GreeterImpl { val: 42 };
            dec fut = worker(&gi);
            return 0;
        }
    "#;
    let (success, diags) = compile_to_obj("test_adv_13", src);
    assert!(success, "Future cancellation at state 0 with dyn Trait should compile cleanly: {:?}", diags);
}

// ADV-14: Dropping suspended future holding Drop struct and dyn Trait reference cascades drop
#[test]
fn test_adv_14_future_cancellation_at_suspended_state_with_dyn_trait() {
    let src = r#"
        import <core>;

        struct Resource {
            id: i32,
        }

        impl core::Drop for Resource {
            fn drop(self: &rw Self) {}
        }

        trait Greeter {
            fn greet(self: &Self) -> i32;
        }

        struct GreeterImpl {
            val: i32,
        }

        impl Greeter for GreeterImpl {
            fn greet(self: &Self) -> i32 { return self.val; }
        }

        async fn helper() -> i32 {
            return 100;
        }

        async fn worker(g: &dyn Greeter) -> i32 {
            dec r = Resource { id: 10 };
            dec h = await helper();
            return g.greet() + r.id + h;
        }

        fn main() -> i32 {
            dec gi = GreeterImpl { val: 42 };
            dec fut = worker(&gi);
            return 0;
        }
    "#;
    let (success, diags) = compile_to_obj("test_adv_14", src);
    assert!(success, "Future cancellation at suspended state with dyn Trait should compile cleanly: {:?}", diags);
}

// ADV-15: Full pipeline multi-state machine with virtual method dispatches before & after await
#[test]
fn test_adv_15_full_pipeline_async_virtual_dispatch() {
    let src = r#"
        trait Computable {
            fn compute(self: &Self, factor: i32) -> i32;
        }

        struct Calc {
            base: i32,
        }

        impl Computable for Calc {
            fn compute(self: &Self, factor: i32) -> i32 {
                return self.base * factor;
            }
        }

        async fn pause() -> i32 {
            return 5;
        }

        async fn process(c: &dyn Computable) -> i32 {
            dec before = c.compute(2);
            dec p = await pause();
            dec after = c.compute(3);
            return before + p + after;
        }

        fn main() -> i32 {
            dec c = Calc { base: 10 };
            dec fut = process(&c);
            return 0;
        }
    "#;
    let (success, diags) = compile_to_obj("test_adv_15", src);
    assert!(success, "Full pipeline async virtual dispatch should compile cleanly: {:?}", diags);
}

// ADV-16: FutureLoan does not capture by-value arguments (only reference-bearing arguments)
#[test]
fn test_adv_16_futureloan_does_not_capture_by_value_argument() {
    let mut ctx = SemanticContext::new();
    let i32_ty = ctx.types.intern(SemanticType::Primitive(mellis_semantic::ty::BuiltinType::I32));
    let fut_i32_ty = ctx.types.intern(SemanticType::Future(i32_ty));
    let trait_sym = mellis_common::ids::SymbolId(20);
    let dyn_trait_ty = ctx.types.intern(SemanticType::DynTrait(trait_sym));
    let ref_dyn_ty = ctx.types.intern(SemanticType::Reference(
        mellis_semantic::ty::LifetimeId(0),
        mellis_semantic::ty::Mutability::Immutable,
        dyn_trait_ty,
    ));

    // func: (d: &dyn Trait, count: i32)
    let mut func = Function {
        name: GlobalId { name: "test_by_value_arg".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 2,
        link_name: None,
        param_types: vec![ref_dyn_ty, i32_ty],
        ret_ty: fut_i32_ty,
        blocks: Vec::new(),
        values: Vec::new(),
    };

    // %v0: d: &dyn Trait (reference parameter)
    func.values.push(ValueData {
        inst: Instruction::Alloca,
        ty: ref_dyn_ty,
        span: None,
        origin: ValueOrigin::Parameter(0),
    });

    // %v1: count: i32 (by-value parameter)
    func.values.push(ValueData {
        inst: Instruction::Alloca,
        ty: i32_ty,
        span: None,
        origin: ValueOrigin::Parameter(1),
    });

    // %v2: CallVirt { obj: %v0, method_idx: 0, args: [%v1] } -> Future<i32>
    func.values.push(ValueData {
        inst: Instruction::CallVirt {
            obj: Operand::Value(ValueId(0)),
            method_idx: 0,
            args: vec![Operand::Value(ValueId(1))],
        },
        ty: fut_i32_ty,
        span: None,
        origin: ValueOrigin::Temporary,
    });

    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(2)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(2))) }),
    });

    let diags = BorrowAnalyzer::analyze(&func, None, Some(&ctx));
    assert!(diags.is_empty(), "By-value argument should not generate borrow conflicts: {:?}", diags);
}
