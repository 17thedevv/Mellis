use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_04_{}", name));
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

/// C-GAP-04: Test 1
/// Verifies that generic struct parameter with parameterized trait bound `Tr: Transformer<In, Out>`
/// correctly propagates both generic arguments `In` and `Out` where `In != Out` (two distinct types).
#[test]
fn test_c_gap_04_parameterized_trait_bound_distinct_args() {
    let src = r#"
trait Transformer<In, Out> {
    fn transform(self: &Self, val: In) -> Out;
}

struct IntToLongWorker {
    factor: i64,
};

impl Transformer<i32, i64> for IntToLongWorker {
    fn transform(self: &Self, val: i32) -> i64 {
        return (val as i64) * self.factor;
    }
}

struct Pipeline<In, Out, Tr: Transformer<In, Out>> {
    worker: Tr,
};

impl<In, Out, Tr: Transformer<In, Out>> Pipeline<In, Out, Tr> {
    pub fn process(self: &Self, val: In) -> Out {
        return self.worker.transform(val);
    }
}

fn main() -> i32 {
    dec worker = IntToLongWorker { factor: 10 as i64 };
    dec pipeline = Pipeline<i32, i64, IntToLongWorker> {
        worker: worker,
    };
    dec res = pipeline.process(42);
    if res == (420 as i64) {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("distinct_args", src);
    assert_eq!(exit_code, 0, "Pipeline with distinct generic trait args failed: stdout={}, stderr={}", stdout, stderr);
}

/// C-GAP-04: Test 2
/// Verifies parameterized trait bound on generic function `fn execute<In, Out, Tr: Transformer<In, Out>>`.
#[test]
fn test_c_gap_04_generic_function_trait_bound_propagation() {
    let src = r#"
trait Converter<FromTy, ToTy> {
    fn convert(self: &Self, from: FromTy) -> ToTy;
}

struct Doubler {
    base: i32,
};

impl Converter<i32, i32> for Doubler {
    fn convert(self: &Self, from: i32) -> i32 {
        return from * 2 + self.base;
    }
}

fn execute<A, B, C: Converter<A, B>>(c: &C, arg: A) -> B {
    return c.convert(arg);
}

fn main() -> i32 {
    dec d = Doubler { base: 5 };
    dec out = execute<i32, i32, Doubler>(&d, 20);
    if out == 45 {
        return 0;
    }
    return 1;
}
"#;

    let (exit_code, stdout, stderr) = compile_and_run("fn_trait_bound", src);
    assert_eq!(exit_code, 0, "Generic function trait bound failed: stdout={}, stderr={}", stdout, stderr);
}
