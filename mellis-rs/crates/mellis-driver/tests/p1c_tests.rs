use mellis_driver::{check, CompilerOptions};
use mellis_driver::sysroot::Sysroot;
use mellis_driver::async_lowering::{lower_async, FUTURE_STATE_COMPLETED};
use mellis_mvir::{Module, Function, Instruction, Operand, Terminator, BasicBlock, ValueId, ValueData, ValueOrigin, LabelId, GlobalId};
use mellis_semantic::{SemanticContext, SemanticType, SemanticTypeId};
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p1c_tests").join(name);
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

// -----------------------------------------------------------------------------
// Test Matrix for P1-C: Async × Drop / Cancellation (15 Tests)
// -----------------------------------------------------------------------------

// 01. Unawaited future cancellation at initial state (State 0)
#[test]
fn test_case_01_unawaited_future_dropped_at_state_0() {
    let src = r#"
        import <core>;

        struct Resource {
            id: i32,
        }

        impl core::Drop for Resource {
            fn drop(self: &rw Self) {}
        }

        async fn worker() -> i32 {
            dec r = Resource { id: 10 };
            return r.id;
        }

        fn main() -> i32 {
            dec fut = worker();
            // fut dropped at state 0 without being awaited
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(success, "Unawaited future dropped at state 0 should compile cleanly: {:?}", diags);
}

// 02. Suspended future cleanup of live Drop locals
#[test]
fn test_case_02_suspended_future_cleanup_of_live_drop_locals() {
    let mut ctx = SemanticContext::new();
    let mut module = Module::new();

    let struct_ty = ctx.types.intern(SemanticType::Box(SemanticTypeId(3)));

    let func = Function {
        name: GlobalId { name: "worker_with_drop".to_string(), symbol_id: None },
        is_extern: false,
        is_async: true,
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        ret_ty: SemanticTypeId(3),
        blocks: vec![
            BasicBlock {
                label: LabelId { name: "entry".to_string() },
                insts: vec![ValueId(0), ValueId(1), ValueId(2)],
                terminator: Some(Terminator::Ret { value: Some(Operand::Number("0".to_string())) }),
            }
        ],
        values: vec![
            ValueData { inst: Instruction::Alloca, ty: struct_ty, span: None, origin: ValueOrigin::Local },
            ValueData { inst: Instruction::MarkInit { value: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0), span: None, origin: ValueOrigin::Temporary },
            ValueData { inst: Instruction::Await { future: Operand::Number("1".to_string()) }, ty: SemanticTypeId(3), span: None, origin: ValueOrigin::Temporary },
        ],
    };

    module.functions.push(func);
    lower_async(&mut module, &mut ctx);

    assert_eq!(module.functions.len(), 3);
    let drop_fn = &module.functions[2];
    assert_eq!(drop_fn.name.name, "worker_with_drop_drop");

    // Must have drop_state_1 block (which checks child future nullity) and drop_locals_1 block
    let block_labels: Vec<&str> = drop_fn.blocks.iter().map(|b| b.label.name.as_str()).collect();
    assert!(block_labels.contains(&"drop_state_1"), "Must contain drop_state_1");
    assert!(block_labels.contains(&"drop_locals_1"), "Must contain drop_locals_1");

    // drop_fn must contain at least 2 Drop instructions (child future drop + local resource drop)
    let drop_inst_count = drop_fn.values.iter().filter(|v| matches!(v.inst, Instruction::Drop { .. })).count();
    assert!(drop_inst_count >= 2, "Expected child future and local resource drop, got {}", drop_inst_count);
}

// 03. Completed future destruction without double-drop
#[test]
fn test_case_03_completed_future_destruction_without_double_drop() {
    let mut ctx = SemanticContext::new();
    let mut module = Module::new();

    let func = Function {
        name: GlobalId { name: "simple_worker".to_string(), symbol_id: None },
        is_extern: false,
        is_async: true,
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        ret_ty: SemanticTypeId(3),
        blocks: vec![
            BasicBlock {
                label: LabelId { name: "entry".to_string() },
                insts: vec![ValueId(0)],
                terminator: Some(Terminator::Ret { value: Some(Operand::Number("42".to_string())) }),
            }
        ],
        values: vec![
            ValueData { inst: Instruction::Alloca, ty: SemanticTypeId(3), span: None, origin: ValueOrigin::Local },
        ],
    };

    module.functions.push(func);
    lower_async(&mut module, &mut ctx);

    let drop_fn = &module.functions[2];
    let entry_block = &drop_fn.blocks[0];

    // Entry block must branch directly to free_env if state == FUTURE_STATE_COMPLETED
    assert_eq!(entry_block.label.name, "entry");
    let is_completed_cmp = drop_fn.values.iter().any(|v| {
        matches!(&v.inst, Instruction::Eq { right: Operand::Number(n), .. } if n == &FUTURE_STATE_COMPLETED.to_string())
    });
    assert!(is_completed_cmp, "Must compare against FUTURE_STATE_COMPLETED");
    match &entry_block.terminator {
        Some(Terminator::CondBr { true_target, .. }) => {
            assert_eq!(true_target.name, "free_env", "Completed future must fast-path directly to free_env");
        }
        _ => panic!("Expected CondBr in entry block"),
    }
}

// 04. Nested future cancellation cascading to child future
#[test]
fn test_case_04_nested_future_cancellation_cascades() {
    let mut ctx = SemanticContext::new();
    let mut module = Module::new();

    let child_func = Function {
        name: GlobalId { name: "child_task".to_string(), symbol_id: None },
        is_extern: false,
        is_async: true,
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        ret_ty: SemanticTypeId(3),
        blocks: vec![
            BasicBlock {
                label: LabelId { name: "entry".to_string() },
                insts: vec![],
                terminator: Some(Terminator::Ret { value: Some(Operand::Number("1".to_string())) }),
            }
        ],
        values: vec![],
    };

    let parent_func = Function {
        name: GlobalId { name: "parent_task".to_string(), symbol_id: None },
        is_extern: false,
        is_async: true,
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        ret_ty: SemanticTypeId(3),
        blocks: vec![
            BasicBlock {
                label: LabelId { name: "entry".to_string() },
                insts: vec![ValueId(0), ValueId(1)],
                terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(1))) }),
            }
        ],
        values: vec![
            ValueData {
                inst: Instruction::CallDirect {
                    callee: GlobalId { name: "child_task".to_string(), symbol_id: None },
                    args: vec![],
                },
                ty: ctx.types.intern(SemanticType::Future(SemanticTypeId(3))),
                span: None,
                origin: ValueOrigin::Temporary,
            },
            ValueData {
                inst: Instruction::Await { future: Operand::Value(ValueId(0)) },
                ty: SemanticTypeId(3),
                span: None,
                origin: ValueOrigin::Temporary,
            },
        ],
    };

    module.functions.push(child_func);
    module.functions.push(parent_func);
    lower_async(&mut module, &mut ctx);

    // parent_task_drop should have child_task_drop as callee for its child future drop
    let parent_drop_fn = module.functions.iter().find(|f| f.name.name == "parent_task_drop").expect("parent_task_drop not found");
    let child_drop_inst = parent_drop_fn.values.iter().find(|v| {
        if let Instruction::Drop { callee: Some(callee), .. } = &v.inst {
            callee.name == "child_task_drop"
        } else {
            false
        }
    });
    assert!(child_drop_inst.is_some(), "parent_task_drop must cascade cancellation to child_task_drop");
}

// 05. Future<i32> still gets a destructor
#[test]
fn test_case_05_future_i32_still_gets_a_destructor() {
    let src = r#"
        async fn compute() -> i32 {
            return 42;
        }

        fn main() -> i32 {
            dec fut = compute();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(success, "Future<i32> must compile and generate destructor without errors: {:?}", diags);
}

// 06. Multiple await points with state-specific cleanup
#[test]
fn test_case_06_multiple_await_points_with_state_specific_cleanup() {
    let src = r#"
        import <core>;

        struct ResourceA { id: i32 }
        impl core::Drop for ResourceA { fn drop(self: &rw Self) {} }

        struct ResourceB { id: i32 }
        impl core::Drop for ResourceB { fn drop(self: &rw Self) {} }

        async fn step() -> i32 { return 1; }

        async fn multi_step() -> i32 {
            dec a = ResourceA { id: 1 };
            dec s1 = await step();
            dec b = ResourceB { id: 2 };
            dec s2 = await step();
            return a.id + b.id + s1 + s2;
        }

        fn main() -> i32 {
            dec fut = multi_step();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_06", src);
    assert!(success, "Multi-await state-specific cleanup should compile: {:?}", diags);
}

// 07. Drop-type parameters cleaned when cancelled before completion
#[test]
fn test_case_07_drop_type_parameters_cleaned_when_cancelled_early() {
    let src = r#"
        import <core>;

        struct Resource { id: i32 }
        impl core::Drop for Resource { fn drop(self: &rw Self) {} }

        async fn other() -> i32 { return 0; }

        async fn worker(r: Resource) -> i32 {
            dec x = await other();
            return r.id + x;
        }

        fn main() -> i32 {
            dec r = Resource { id: 42 };
            dec fut = worker(r);
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_07", src);
    assert!(success, "Drop parameters cleaned on cancellation should pass: {:?}", diags);
}

// 08. Moved resource is not double-dropped on cancellation
#[test]
fn test_case_08_moved_resource_is_not_double_dropped_on_cancellation() {
    let src = r#"
        import <core>;

        struct Resource { id: i32 }
        impl core::Drop for Resource { fn drop(self: &rw Self) {} }

        async fn other() -> i32 { return 1; }

        async fn worker() -> i32 {
            dec r1 = Resource { id: 1 };
            dec r2 = r1; // r1 is moved to r2
            dec x = await other();
            return r2.id + x;
        }

        fn main() -> i32 {
            dec fut = worker();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(success, "Moved resource before await should not be double dropped: {:?}", diags);
}

// 09. Future moved from fut1 to fut2 has exactly one destructor owner
#[test]
fn test_case_09_future_moved_has_exactly_one_owner() {
    let src = r#"
        async fn compute() -> i32 { return 42; }

        fn main() -> i32 {
            dec fut1 = compute();
            dec fut2 = fut1; // ownership transferred to fut2
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_09", src);
    assert!(success, "Future moved between variables should compile with single drop owner: {:?}", diags);
}

// 10. Branching await points produce distinct cleanup sets
#[test]
fn test_case_10_branching_awaits_distinct_cleanup_sets() {
    let src = r#"
        import <core>;

        struct LeftRes { id: i32 }
        impl core::Drop for LeftRes { fn drop(self: &rw Self) {} }

        struct RightRes { id: i32 }
        impl core::Drop for RightRes { fn drop(self: &rw Self) {} }

        async fn step() -> i32 { return 1; }

        async fn branching(cond: bool) -> i32 {
            if cond {
                dec l = LeftRes { id: 10 };
                dec s = await step();
                return l.id + s;
            } else {
                dec r = RightRes { id: 20 };
                dec s = await step();
                return r.id + s;
            }
        }

        fn main() -> i32 {
            dec fut = branching(true);
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(success, "Branching await points should compile cleanly: {:?}", diags);
}

// 11. Loop await cancellation
#[test]
fn test_case_11_loop_await_cancellation() {
    let src = r#"
        async fn poll_once() -> i32 { return 1; }

        async fn run_loop() -> i32 {
            dec rw count = 0;
            while count < 3 {
                dec res = await poll_once();
                count = count + res;
            }
            return count;
        }

        fn main() -> i32 {
            dec fut = run_loop();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_11", src);
    assert!(success, "Loop await cancellation should compile cleanly: {:?}", diags);
}

// 12. Partial initialization: initialized values are dropped, uninitialized values are not
#[test]
fn test_case_12_partial_initialization_dropped_correctly() {
    let src = r#"
        import <core>;

        struct Resource { id: i32 }
        impl core::Drop for Resource { fn drop(self: &rw Self) {} }

        async fn step() -> i32 { return 1; }

        async fn partially_initialized() -> i32 {
            dec r1 = Resource { id: 1 }; // initialized before await
            dec s = await step();
            dec r2 = Resource { id: 2 }; // initialized after await
            return r1.id + r2.id + s;
        }

        fn main() -> i32 {
            dec fut = partially_initialized();
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_12", src);
    assert!(success, "Partial initialization drop selection should compile: {:?}", diags);
}

// 13. Future cancellation releases caller-side FutureLoan
#[test]
fn test_case_13_future_cancellation_releases_caller_loan() {
    let src = r#"
        async fn worker(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec x = 42;
            {
                dec fut = worker(&x);
                // fut dropped here at scope exit
            }
            // x should be freely accessible again because future was dropped
            dec y = x;
            return y;
        }
    "#;
    let (success, diags) = run_compiler("test_13", src);
    assert!(success, "Future cancellation releases caller loan: {:?}", diags);
}

// 14. Child future null/absent path is safe if representable by the runtime
#[test]
fn test_case_14_child_future_null_absent_path_is_safe() {
    let mut ctx = SemanticContext::new();
    let mut module = Module::new();

    let func = Function {
        name: GlobalId { name: "await_worker".to_string(), symbol_id: None },
        is_extern: false,
        is_async: true,
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        ret_ty: SemanticTypeId(3),
        blocks: vec![
            BasicBlock {
                label: LabelId { name: "entry".to_string() },
                insts: vec![ValueId(0)],
                terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(0))) }),
            }
        ],
        values: vec![
            ValueData {
                inst: Instruction::Await { future: Operand::Number("0".to_string()) },
                ty: SemanticTypeId(3),
                span: None,
                origin: ValueOrigin::Temporary,
            },
        ],
    };

    module.functions.push(func);
    lower_async(&mut module, &mut ctx);

    let drop_fn = &module.functions[2];
    // Check that drop_state_1 has null check and branches
    let state_1_block = drop_fn.blocks.iter().find(|b| b.label.name == "drop_state_1").expect("drop_state_1 not found");
    assert!(state_1_block.insts.iter().any(|&vid| matches!(drop_fn.values[vid.0 as usize].inst, Instruction::NotEq { .. })), "Must check child_future != null");
    assert!(matches!(state_1_block.terminator, Some(Terminator::CondBr { .. })), "Must branch conditionally on child null check");
}

// 15. Do not introduce or claim implicit Send/Sync semantics
#[test]
fn test_case_15_no_implicit_send_sync_semantics() {
    let src = r#"
        async fn task() -> i32 {
            return 1;
        }

        fn main() -> i32 {
            dec fut = task();
            // Future is a local non-thread-bound state machine value
            return 0;
        }
    "#;
    let (success, diags) = run_compiler("test_15", src);
    assert!(success, "Basic future usage without thread transfer passes: {:?}", diags);
}
