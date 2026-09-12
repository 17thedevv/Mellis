use luna_driver::{check, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_p2a_tests").join(name);
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

    run_compiler_with_options(name, src, options)
}

fn run_compiler_with_options(name: &str, src: &str, options: CompilerOptions) -> (bool, Vec<luna_common::Diagnostic>) {
    let temp = create_temp_dir(name);
    let main_path = temp.join("main.ln");
    fs::write(&main_path, src).unwrap();

    let res = check(main_path.to_str().unwrap(), src.to_string(), &options);
    match res {
        Ok(_) => (true, vec![]),
        Err(diags) => (false, diags),
    }
}

// 01. Pure arithmetic in const declaration compiles and evaluates
#[test]
fn test_case_01_const_pure_arithmetic() {
    let src = r#"
        const BASE: i32 = 100;
        const OFFSET: i32 = 28;
        const TOTAL: i32 = BASE + OFFSET;

        fn main() -> i32 {
            TOTAL
        }
    "#;
    let (success, diags) = run_compiler("test_01", src);
    assert!(success, "Expected successful compilation, got: {:?}", diags);
}

// 02. Out-of-order const dependencies evaluate in topological order
#[test]
fn test_case_02_const_out_of_order_dependency() {
    let src = r#"
        const C: i32 = B * 2;
        const B: i32 = A + 5;
        const A: i32 = 10;

        fn main() -> i32 {
            C
        }
    "#;
    let (success, diags) = run_compiler("test_02", src);
    assert!(success, "Expected topological sort to succeed, got: {:?}", diags);
}

// 03. Cyclic dependency between two constants is detected and rejected
#[test]
fn test_case_03_const_dependency_cycle_detected() {
    let src = r#"
        const A: i32 = B + 1;
        const B: i32 = A + 1;

        fn main() -> i32 {
            A
        }
    "#;
    let (success, diags) = run_compiler("test_03", src);
    assert!(!success, "Expected cycle detection error, but compilation succeeded");

    let cycle_err = diags.iter().find(|d| d.message.contains("cycle detected in constant evaluation"));
    assert!(cycle_err.is_some(), "Expected E_CONST_DEPENDENCY_CYCLE diagnostic, got: {:?}", diags);
}

// 04. Three-node cyclic dependency is detected
#[test]
fn test_case_04_const_three_node_cycle_detected() {
    let src = r#"
        const X: i32 = Y;
        const Y: i32 = Z;
        const Z: i32 = X;

        fn main() -> i32 {
            X
        }
    "#;
    let (success, diags) = run_compiler("test_04", src);
    assert!(!success, "Expected 3-node cycle detection error, but compilation succeeded");

    let cycle_err = diags.iter().find(|d| d.message.contains("cycle detected in constant evaluation"));
    assert!(cycle_err.is_some(), "Expected cycle diagnostic, got: {:?}", diags);
}

// 05. Const admission rejects raw function calls without comptime
#[test]
fn test_case_05_const_admission_rejects_raw_call() {
    let src = r#"
        fn helper() -> i32 {
            42
        }
        const BAD: i32 = helper();

        fn main() -> i32 {
            BAD
        }
    "#;
    let (success, diags) = run_compiler("test_05", src);
    assert!(!success, "Expected rejection of function call in raw const");

    let err = diags.iter().find(|d| d.message.contains("function calls in const initializers are forbidden"));
    assert!(err.is_some(), "Expected admission error regarding function call, got: {:?}", diags);
}

// 06. Local const declarations inside functions are evaluated
#[test]
fn test_case_06_local_const_declaration_evaluated() {
    let src = r#"
        fn compute() -> i32 {
            const LOCAL_VAL: i32 = 40 + 2;
            LOCAL_VAL
        }

        fn main() -> i32 {
            compute()
        }
    "#;
    let (success, diags) = run_compiler("test_06", src);
    assert!(success, "Expected local const evaluation to succeed, got errors: {:?}", diags);
}

// 07. Embedded comptime blocks in const declarations are permitted
#[test]
fn test_case_07_const_admission_allows_embedded_comptime() {
    let src = r#"
        const COMPUTED: i32 = comptime {
            dec a = 15;
            dec b = 25;
            a + b
        };

        fn main() -> i32 {
            COMPUTED
        }
    "#;
    let (success, diags) = run_compiler("test_07", src);
    assert!(success, "Expected embedded comptime in const to succeed, got errors: {:?}", diags);
}

// 08. Comptime execution with local mutable state and while loop
#[test]
fn test_case_08_comptime_while_loop_mutation() {
    let src = r#"
        const FACTORIAL_5: i32 = comptime {
            dec rw result = 1;
            dec rw i = 1;
            while i <= 5 {
                result = result * i;
                i = i + 1;
            }
            result
        };

        fn main() -> i32 {
            FACTORIAL_5
        }
    "#;
    let (success, diags) = run_compiler("test_08", src);
    assert!(success, "Expected comptime while loop to compute factorial, got: {:?}", diags);
}

// 09. Comptime calling a user-defined pure function
#[test]
fn test_case_09_comptime_calls_pure_function() {
    let src = r#"
        fn square(x: i32) -> i32 {
            x * x
        }

        const VAL: i32 = comptime {
            square(7)
        };

        fn main() -> i32 {
            VAL
        }
    "#;
    let (success, diags) = run_compiler("test_09", src);
    assert!(success, "Expected comptime calling pure function to succeed, got: {:?}", diags);
}

// 10. Comptime referencing a const item (intra-item dependency)
#[test]
fn test_case_10_comptime_references_const() {
    let src = r#"
        const BASE: i32 = 50;
        const DERIVED: i32 = comptime {
            BASE + 25
        };

        fn main() -> i32 {
            DERIVED
        }
    "#;
    let (success, diags) = run_compiler("test_10", src);
    assert!(success, "Expected comptime referencing const to succeed, got: {:?}", diags);
}

// 11. Cycle detected across embedded comptime blocks
#[test]
fn test_case_11_comptime_cycle_detected() {
    let src = r#"
        const A: i32 = comptime {
            B + 1
        };
        const B: i32 = comptime {
            A + 1
        };

        fn main() -> i32 {
            A
        }
    "#;
    let (success, diags) = run_compiler("test_11", src);
    assert!(!success, "Expected cycle detection error in comptime blocks");
    let cycle_err = diags.iter().find(|d| d.message.contains("cycle detected in constant evaluation"));
    assert!(cycle_err.is_some(), "Expected E_CONST_DEPENDENCY_CYCLE diagnostic, got: {:?}", diags);
}

// 12. Division by zero in comptime is caught at compile time with NO runtime fallback
#[test]
fn test_case_12_comptime_div_by_zero_fails_without_fallback() {
    let src = r#"
        const BAD: i32 = comptime {
            100 / 0
        };

        fn main() -> i32 {
            BAD
        }
    "#;
    let (success, diags) = run_compiler("test_12", src);
    assert!(!success, "Expected compile-time error for comptime division by zero");
    let div_err = diags.iter().find(|d| d.message.contains("divide by zero"));
    assert!(div_err.is_some(), "Expected division by zero error, got: {:?}", diags);
}

// 13. Step limit exhaustion triggers E_COMPTIME_STEP_LIMIT with zero runtime fallback
#[test]
fn test_case_13_comptime_step_limit_exceeded() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
        const INFINITE: i32 = comptime {
            dec rw x = 0;
            while x >= 0 {
                x = x + 1;
            }
            x
        };

        fn main() -> i32 {
            INFINITE
        }
    "#;
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        comptime_steps: Some(50),
        ..Default::default()
    };
    let (success, diags) = run_compiler_with_options("test_13", src, options);
    assert!(!success, "Expected step limit failure");
    let limit_err = diags.iter().find(|d| d.message.contains("E_COMPTIME_STEP_LIMIT"));
    assert!(limit_err.is_some(), "Expected E_COMPTIME_STEP_LIMIT error, got: {:?}", diags);
}

// 14. Call recursion depth limit triggers E_COMPTIME_RECURSION_LIMIT
#[test]
fn test_case_14_comptime_recursion_limit_exceeded() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let src = r#"
        fn recurse(n: i32) -> i32 {
            recurse(n + 1)
        }

        const DEEP: i32 = comptime {
            recurse(0)
        };

        fn main() -> i32 {
            DEEP
        }
    "#;
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        comptime_depth: Some(10),
        ..Default::default()
    };
    let (success, diags) = run_compiler_with_options("test_14", src, options);
    assert!(!success, "Expected recursion limit failure");
    let depth_err = diags.iter().find(|d| d.message.contains("E_COMPTIME_RECURSION_LIMIT"));
    assert!(depth_err.is_some(), "Expected E_COMPTIME_RECURSION_LIMIT error, got: {:?}", diags);
}

// 15. Pointer escape from comptime is rejected with E_COMPTIME_POINTER_ESCAPE
#[test]
fn test_case_15_comptime_pointer_escape_rejected() {
    let src = r#"
        const ESCAPED: *i32 = comptime {
            dec x = 42;
            &x
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_15", src);
    assert!(!success, "Expected pointer escape to be rejected");
    let esc_err = diags.iter().find(|d| d.message.contains("E_COMPTIME_POINTER_ESCAPE") || d.message.contains("pointer escape"));
    assert!(esc_err.is_some(), "Expected pointer escape error, got: {:?}", diags);
}

// 16. Comptime expressions directly embedded inside runtime functions materialize correctly
#[test]
fn test_case_16_runtime_embedded_comptime_materialization() {
    let src = r#"
        fn main() -> i32 {
            dec x = comptime {
                dec rw acc = 0;
                dec rw i = 0;
                while i < 10 {
                    acc = acc + i;
                    i = i + 1;
                }
                acc
            };
            x
        }
    "#;
    let (success, diags) = run_compiler("test_16", src);
    assert!(success, "Expected runtime embedded comptime to succeed and materialize, got errors: {:?}", diags);
}

// 17. Effect containment rejects async await in comptime
#[test]
fn test_case_17_comptime_await_forbidden() {
    let src = r#"
        async fn async_val() -> i32 {
            10
        }

        const BAD_ASYNC: i32 = comptime {
            async_val().await
        };

        fn main() -> i32 {
            BAD_ASYNC
        }
    "#;
    let (success, diags) = run_compiler("test_17", src);
    assert!(!success, "Expected await in comptime to be rejected");
    let await_err = diags.iter().find(|d| d.message.contains("await") || d.message.contains("comptime await"));
    assert!(await_err.is_some(), "Expected await forbidden error, got: {:?}", diags);
}

// 18. Closure escape from comptime is rejected with E_COMPTIME_RESOURCE_ESCAPE
#[test]
fn test_case_18_comptime_closure_escape_rejected() {
    let src = r#"
        const CLOSURE = comptime {
            |x: i32| -> i32 { x + 1 }
        };

        fn main() -> i32 {
            0
        }
    "#;
    let (success, diags) = run_compiler("test_18", src);
    assert!(!success, "Expected closure escape to be rejected");
    let esc_err = diags.iter().find(|d| d.message.contains("E_COMPTIME_RESOURCE_ESCAPE") || d.message.contains("E_COMPTIME_RESOURCE_LEAK") || d.message.contains("cannot export closure") || d.message.contains("closures and lambdas cannot be evaluated"));
    assert!(esc_err.is_some(), "Expected resource escape error, got: {:?}", diags);
}

// 19. Reading moved value in compile-time evaluation is rejected with E_USE_OF_MOVED_VALUE
#[test]
fn test_case_19_comptime_use_of_moved_value() {
    let src = r#"
        struct NonCopy {
            val: i32,
        }

        const BAD_MOVE: i32 = comptime {
            dec a = NonCopy { val: 10 };
            dec b = a;
            dec c = a;
            c.val
        };

        fn main() -> i32 {
            BAD_MOVE
        }
    "#;
    let (success, diags) = run_compiler("test_19", src);
    assert!(!success, "Expected use of moved value to be rejected in comptime");
    let move_err = diags.iter().find(|d| d.message.contains("E_USE_OF_MOVED_VALUE") || d.message.contains("use of moved value"));
    assert!(move_err.is_some(), "Expected E_USE_OF_MOVED_VALUE error, got: {:?}", diags);
}

// 20. Resource leak: unfreed compile-time heap memory is rejected with E_COMPTIME_RESOURCE_LEAK
#[test]
fn test_case_20_comptime_resource_leak() {
    use luna_mvir::interp::MvirInterpreter;
    use luna_mvir::mvir::*;
    use luna_semantic::SemanticContext;

    let ctx = SemanticContext::new();
    let mut func = Function {
        name: GlobalId { name: "test_leak".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 0,
        link_name: None,
        param_types: Vec::new(),
        ret_ty: luna_semantic::SemanticTypeId(0),
        blocks: Vec::new(),
        values: Vec::new(),
    };
    let vid = ValueId(0);
    func.values.push(ValueData {
        ty: luna_semantic::SemanticTypeId(0),
        inst: Instruction::HeapAlloc,
        origin: ValueOrigin::Temporary,
        span: None,
    });
    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![vid],
        terminator: Some(Terminator::Ret { value: None }),
    });

    let module = Module {
        functions: vec![func.clone()],
    };

    let mut interp = MvirInterpreter::new(&module, &ctx);
    let _ = interp.eval_function(&func, Vec::new()).unwrap();
    assert!(!interp.heap.allocations.is_empty(), "Expected heap allocation to remain unfreed");

    let leak_check = if !interp.heap.allocations.is_empty() {
        Err(luna_semantic::ComptimeError::ResourceLeak("unfreed heap allocation".to_string()))
    } else {
        Ok(())
    };
    assert!(leak_check.is_err(), "Expected resource leak error");
    let err_str = format!("{}", leak_check.unwrap_err());
    assert!(err_str.contains("E_COMPTIME_RESOURCE_LEAK"), "Expected E_COMPTIME_RESOURCE_LEAK in: {}", err_str);
}


