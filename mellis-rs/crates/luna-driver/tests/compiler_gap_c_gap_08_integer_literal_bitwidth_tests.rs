use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_08_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compile_and_run(test_name: &str, source: &str) -> (i32, String, String) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    let exe_path = temp.join(format!("{}.exe", test_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), source.to_string(), &options);
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// C-GAP-08: Full 64-bit integer literal precision in LLVM codegen without 32-bit truncation
#[test]
fn test_c_gap_08_64bit_literal_preservation() {
    let src = r#"
fn takes_u64(x: u64) -> u64 {
    return x;
}

fn returns_large_u64() -> u64 {
    return 18446744071991564697 as u64;
}

fn main() -> i32 {
    // 1. Zero-extension of unsigned cast: u8 (255) to u64 must be 255, not sign-extended -1
    dec u_byte: u8 = 255 as u8;
    dec u_ext: u64 = u_byte as u64;
    if u_ext != (255 as u64) {
        return 1;
    }

    // 2. Sign-extension of signed cast: i8 (-1) to i64 must be -1
    dec s_byte: i8 = (0 as i8) - (1 as i8);
    dec s_ext: i64 = s_byte as i64;
    if s_ext != ((0 as i64) - (1 as i64)) {
        return 2;
    }

    // 3. i64::MAX literal in assignment and comparison
    dec max_i64: i64 = 9223372036854775807 as i64;
    if max_i64 <= (2147483647 as i64) {
        return 3;
    }

    // 4. i64::MIN path: underflow / signed negation preserved
    dec min_i64: i64 = (0 as i64) - (9223372036854775807 as i64) - (1 as i64);
    if min_i64 >= (0 as i64) {
        return 4;
    }

    // 5. u64 large literal (> 32 bits and > i64::MAX)
    dec large_u64: u64 = 18446744071991564697 as u64;
    dec upper = large_u64 >> (32 as u64);
    if upper == (0 as u64) {
        return 5;
    }

    // 6. Literal in assignment with 64-bit value
    dec a: u64 = 5000000000 as u64;
    if a != (5000000000 as u64) {
        return 6;
    }

    // 7. Literal in cast from large constant
    dec b: u64 = (5000000000 as u64);
    if b != (5000000000 as u64) {
        return 7;
    }

    // 8. Literal in arithmetic (add, mul) exceeding 32-bit range
    dec c: u64 = 5000000000 as u64;
    dec d = c + (1000000000 as u64);
    if d != (6000000000 as u64) {
        return 8;
    }
    dec e = d * (2 as u64);
    if e != (12000000000 as u64) {
        return 9;
    }

    // 9. Literal as function argument
    dec res_arg = takes_u64(18446744071991564697 as u64);
    if res_arg != (18446744071991564697 as u64) {
        return 10;
    }

    // 10. Literal as return value
    dec res_ret = returns_large_u64();
    if res_ret != (18446744071991564697 as u64) {
        return 11;
    }

    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("test_c_gap_08_64bit_literal_preservation", src);
    assert_eq!(code, 0, "Test failed with exit code: {}\nstdout: {}\nstderr: {}", code, stdout, stderr);
}

/// Verify that raw Operand::Number directly passed to MVIR instructions (Add, Ret) without
/// preceding Assign or Cast is correctly lowered to the target bitwidth (e.g. i64) rather
/// than being implicitly truncated to i32.
#[test]
fn test_c_gap_08_mvir_direct_typed_operand_lowering() {
    use luna_mvir::{Module, Function, BasicBlock, Instruction, Terminator, Operand, ValueId, ValueData, LabelId, GlobalId};
    use luna_semantic::{SemanticContext, ty::{BuiltinType, SemanticType}};
    use luna_backend::LLVMBackend;
    use inkwell::context::Context;

    let mut semantic_ctx = SemanticContext::new();
    let u64_ty = semantic_ctx.types.intern(SemanticType::Primitive(BuiltinType::U64));

    let mut func = Function {
        name: GlobalId { name: "test_direct_literal_math".to_string(), symbol_id: None },
        is_extern: false,
        is_async: false,
        arg_count: 1,
        link_name: None,
        param_types: vec![u64_ty],
        ret_ty: u64_ty,
        blocks: vec![],
        values: vec![],
    };

    // Value 0: Parameter alloca slot for param 0
    func.values.push(ValueData::new(
        Instruction::Alloca,
        u64_ty,
        None,
    ));

    // Value 1: Load param
    func.values.push(ValueData::new(
        Instruction::Load { ptr: Operand::Value(ValueId(0)) },
        u64_ty,
        None,
    ));

    // Value 2: Add with raw Operand::Number literal exceeding 32-bit range
    func.values.push(ValueData::new(
        Instruction::Add {
            left: Operand::Value(ValueId(1)),
            right: Operand::Number("5000000000".to_string()),
        },
        u64_ty,
        None,
    ));

    // Block 0: entry block with Add instruction and Ret with Add result
    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![ValueId(0), ValueId(1), ValueId(2)],
        terminator: Some(Terminator::Ret {
            value: Some(Operand::Value(ValueId(2))),
        }),
    });

    let mut module = Module::new();
    module.functions.push(func);

    let llvm_ctx = Context::create();
    let mut backend = LLVMBackend::new(&llvm_ctx, &module, &semantic_ctx, "c_gap_08_test");
    assert!(backend.compile().is_ok(), "LLVMBackend compilation failed");

    let ir = backend.to_llvm_ir();
    // Verify that the constant 5000000000 in Add is lowered as 64-bit integer constant i64, NOT truncated to i32!
    assert!(ir.contains("5000000000"), "IR missing 64-bit constant 5000000000:\n{}", ir);
    assert!(!ir.contains("i32 5000000000"), "IR illegally contains truncated i32 for 64-bit constant:\n{}", ir);
}

