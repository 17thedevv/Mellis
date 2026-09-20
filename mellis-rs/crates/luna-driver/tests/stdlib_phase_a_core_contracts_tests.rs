use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_phase_a_core_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
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

/// 1. Option<T> value methods: is_some, is_none, unwrap, expect, unwrap_or, map, and_then
#[test]
fn test_option_value_methods_e2e() {
    let src = r#"
import <core/panic>;
import <result>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;

fn double_val(x: i32) -> i32 {
    return x * 2;
}

fn opt_reciprocal(x: i32) -> Option<i32> {
    if x == 0 {
        return Option::None;
    }
    return Option::Some(100 / x);
}

fn main() -> i32 {
    // 1. is_some & is_none
    dec some_val: Option<i32> = Option::Some(21);
    dec none_val: Option<i32> = Option::None;

    if some_val.is_some() == false || some_val.is_none() {
        return 1;
    }
    if none_val.is_some() || none_val.is_none() == false {
        return 2;
    }

    // 2. unwrap & expect
    dec u = Option::Some(42).unwrap();
    if u != 42 {
        return 3;
    }
    dec e = Option::Some(84).expect("should have value");
    if e != 84 {
        return 4;
    }

    // 3. unwrap_or
    dec d1 = Option::Some(10).unwrap_or(99);
    dec d2: Option<i32> = Option::None;
    dec d2_val = d2.unwrap_or(99);
    if d1 != 10 || d2_val != 99 {
        return 5;
    }

    // 4. map
    dec mapped = Option::Some(21).map(double_val);
    if mapped.unwrap() != 42 {
        return 6;
    }
    dec mapped_none: Option<i32> = Option::None;
    dec mapped_none_res = mapped_none.map(double_val);
    if mapped_none_res.is_some() {
        return 7;
    }

    // 5. and_then
    dec chain1 = Option::Some(10).and_then(opt_reciprocal);
    if chain1.unwrap() != 10 {
        return 8;
    }
    dec chain2 = Option::Some(0).and_then(opt_reciprocal);
    if chain2.is_some() {
        return 9;
    }
    dec chain_none: Option<i32> = Option::None;
    dec chain3 = chain_none.and_then(opt_reciprocal);
    if chain3.is_some() {
        return 10;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("option_value_methods", src);
    assert_eq!(code, 0, "Option value methods failed with code {}, stderr: {}", code, stderr);
}

/// 2. Result<T, E> value methods: is_ok, is_err, unwrap, expect, unwrap_err, unwrap_or, map, map_err, and_then, OptionExt::ok_or
#[test]
fn test_result_value_methods_e2e() {
    let src = r#"
import <core/panic>;
import <result>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;

fn double_val(x: i32) -> i32 {
    return x * 2;
}

fn negate_err(e: i32) -> i32 {
    return 0 - e;
}

fn res_reciprocal(x: i32) -> Result<i32, i32> {
    if x == 0 {
        return Result::Err(404);
    }
    return Result::Ok(100 / x);
}

fn main() -> i32 {
    // 1. is_ok & is_err
    dec ok_val: Result<i32, i32> = Result::Ok(50);
    dec err_val: Result<i32, i32> = Result::Err(500);

    if ok_val.is_ok() == false || ok_val.is_err() {
        return 1;
    }
    if err_val.is_ok() || err_val.is_err() == false {
        return 2;
    }

    // 2. unwrap & expect & unwrap_err
    dec ok_res1: Result<i32, i32> = Result::Ok(123);
    dec u = ok_res1.unwrap();
    if u != 123 {
        return 3;
    }
    dec ok_res2: Result<i32, i32> = Result::Ok(456);
    dec e = ok_res2.expect("should succeed");
    if e != 456 {
        return 4;
    }
    dec err_code: Result<i32, i32> = Result::Err(999);
    dec unwrapped_err = err_code.unwrap_err();
    if unwrapped_err != 999 {
        return 5;
    }

    // 3. unwrap_or
    dec r1: Result<i32, i32> = Result::Ok(10);
    dec r2: Result<i32, i32> = Result::Err(20);
    if r1.unwrap_or(77) != 10 || r2.unwrap_or(77) != 77 {
        return 6;
    }

    // 4. map & map_err
    dec m_ok: Result<i32, i32> = Result::Ok(25);
    dec m_err: Result<i32, i32> = Result::Err(10);
    dec mapped_ok = m_ok.map(double_val);
    dec mapped_err = m_err.map_err(negate_err);
    if mapped_ok.unwrap() != 50 {
        return 7;
    }
    if mapped_err.unwrap_err() != 0 - 10 {
        return 8;
    }

    // 5. and_then
    dec ch1: Result<i32, i32> = Result::Ok(5);
    dec ch2: Result<i32, i32> = Result::Ok(0);
    dec chained1 = ch1.and_then(res_reciprocal);
    dec chained2 = ch2.and_then(res_reciprocal);
    if chained1.unwrap() != 20 {
        return 9;
    }
    if chained2.unwrap_err() != 404 {
        return 10;
    }

    // 6. OptionExt::ok_or
    dec o_some = Option::Some(88);
    dec o_none: Option<i32> = Option::None;
    dec converted_ok = o_some.ok_or(1);
    dec converted_err = o_none.ok_or(2);
    if converted_ok.unwrap() != 88 || converted_err.unwrap_err() != 2 {
        return 11;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("result_value_methods", src);
    assert_eq!(code, 0, "Result value methods failed with code {}, stderr: {}", code, stderr);
}

/// 3. Iterator contract implementation and execution
#[test]
fn test_iterator_contract_e2e() {
    let src = r#"
import <core/panic>;
import <result>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;

struct Counter {
    current: i32,
    limit: i32,
};

impl Iterator<i32> for Counter {
    fn next(self: &rw Self) -> Option<i32> {
        if self.current < self.limit {
            dec val = self.current;
            self.current = self.current + 1;
            return Option::Some(val);
        }
        return Option::None;
    }
}

fn main() -> i32 {
    dec rw c = Counter { current: 0, limit: 5 };
    dec rw sum = 0;

    dec rw running = true;
    while running {
        dec item = c.next();
        match item {
            Option::Some(v) -> {
                sum = sum + v;
            }
            Option::None -> {
                running = false;
            }
        }
    }

    // sum of 0 + 1 + 2 + 3 + 4 = 10
    if sum != 10 {
        return 1;
    }
    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("iterator_contract", src);
    assert_eq!(code, 0, "Iterator contract failed with code {}, stderr: {}", code, stderr);
}

/// 4. Clone, Eq, Ord, Hash contracts on primitive types
#[test]
fn test_clone_cmp_hash_primitives_e2e() {
    let src = r#"
import <core/panic>;
import <result>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <cmp>;
import <hash>;
import <ptr>;

fn main() -> i32 {
    // 1. Clone
    dec x: i32 = 42;
    dec x_clone = x.clone();
    if x != x_clone {
        return 1;
    }

    // 2. Eq
    dec a: i32 = 100;
    dec b: i32 = 100;
    dec c: i32 = 200;
    if a.eq(&b) == false || a.eq(&c) {
        return 2;
    }

    // 3. Ord
    dec x1: i32 = 10;
    dec x2: i32 = 10;
    dec x3: i32 = 5;
    dec x4: i32 = 20;
    dec cmp_eq = x1.cmp(&x2);
    dec cmp_lt = x3.cmp(&x1);
    dec cmp_gt = x4.cmp(&x1);
    if cmp_eq != 0 || cmp_lt >= 0 || cmp_gt <= 0 {
        return 3;
    }

    // 4. Hash
    dec h_val: u64 = 12345 as u64;
    dec h1 = h_val.hash();
    dec h2 = h_val.hash();
    if h1 != h2 || h1 != 12345 as u64 {
        return 4;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("clone_cmp_hash_primitives", src);
    assert_eq!(code, 0, "Clone/Cmp/Hash contracts failed with code {}, stderr: {}", code, stderr);
}

/// 5. Drop language contract executes on scope exit
#[test]
fn test_drop_language_contract_e2e() {
    let src = r#"
import <core/panic>;
import <result>;
import <mem>;
import <slice>;
import <copy>;
import <ptr>;

struct Sentinel {
    id: i32,
    counter: *rw i32,
};

impl Drop for Sentinel {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = self.id;
        }
    }
}

fn run_scope(counter: *rw i32) {
    dec s = Sentinel { id: 777, counter: counter };
    // s is dropped here upon leaving scope
}

fn main() -> i32 {
    dec rw drop_result: i32 = 0;
    run_scope(&rw drop_result as *rw i32);
    if drop_result != 777 {
        return 1;
    }
    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("drop_language_contract", src);
    assert_eq!(code, 0, "Drop language contract failed with code {}, stderr: {}", code, stderr);
}
