use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_02_{}", name));
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

/// 1. Delimiter Disambiguation: foo::A_B<i32> vs foo::A<i32_B> vs foo_A<B<i32>>
#[test]
fn test_c_gap_02_delimiter_disambiguation() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

module foo {
    export struct i32_B {
        export v: i32,
    }

    export fn A_B<T>(x: T) -> i32 {
        return 10;
    }

    export fn A<T>(x: T) -> i32 {
        return 20;
    }
}

module foo_A {
    export struct B<T> {
        export val: T,
    }

    export fn process<T>(b: B<T>) -> i32 {
        return 30;
    }
}

fn main() -> i32 {
    dec r1 = foo::A_B<i32>(1);
    dec r2 = foo::A<foo::i32_B>(foo::i32_B { v: 2 });
    dec r3 = foo_A::process<i32>(foo_A::B<i32> { val: 3 });

    if r1 + r2 + r3 != 60 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("delimiter_disambiguation", src);
    assert_eq!(code, 0, "Delimiter disambiguation test failed (code: {}, stderr: {})", code, stderr);
}

/// 2. Module Namespace Isolation: foo::process<T> vs bar::process<T> with T=i32
#[test]
fn test_c_gap_02_module_namespace_isolation() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

module foo {
    export fn process<T>(val: T) -> i32 {
        return 100;
    }
}

module bar {
    export fn process<T>(val: T) -> i32 {
        return 200;
    }
}

fn main() -> i32 {
    dec f = foo::process<i32>(10);
    dec b = bar::process<i32>(20);
    if f + b != 300 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("module_namespace_isolation", src);
    assert_eq!(code, 0, "Module namespace isolation failed (code: {}, stderr: {})", code, stderr);
}

/// 3. Inherent Method Name Collision Resistance: TypeA<T>::calc vs TypeB<T>::calc
#[test]
fn test_c_gap_02_method_name_collision_resistance() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

struct TypeA<T> {
    val: T,
}

impl<T> TypeA<T> {
    export fn calc(self: &Self) -> i32 {
        return 111;
    }
}

struct TypeB<T> {
    val: T,
}

impl<T> TypeB<T> {
    export fn calc(self: &Self) -> i32 {
        return 222;
    }
}

fn main() -> i32 {
    dec a = TypeA<i32> { val: 1 };
    dec b = TypeB<i32> { val: 2 };
    if a.calc() + b.calc() != 333 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("method_name_collision_resistance", src);
    assert_eq!(code, 0, "Method collision resistance test failed (code: {}, stderr: {})", code, stderr);
}

/// 4. Same-Name Struct in Different Modules: foo::Box<T> vs bar::Box<T>
#[test]
fn test_c_gap_02_same_name_struct_in_different_modules() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

module foo {
    export struct Box<T> {
        export val: T,
    }

    export fn get_val<T>(b: Box<T>) -> i32 {
        return 50;
    }
}

module bar {
    export struct Box<T> {
        export val: T,
    }

    export fn get_val<T>(b: Box<T>) -> i32 {
        return 70;
    }
}

fn main() -> i32 {
    dec fb = foo::Box<i32> { val: 1 };
    dec bb = bar::Box<i32> { val: 2 };
    if foo::get_val<i32>(fb) + bar::get_val<i32>(bb) != 120 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("same_name_struct_in_different_modules", src);
    assert_eq!(code, 0, "Same-name struct in different modules failed (code: {}, stderr: {})", code, stderr);
}

/// 5. Deeply Nested Generic Substitutions: Outer<Middle<Inner<i32>>>
#[test]
fn test_c_gap_02_deeply_nested_generic_substitutions() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

struct Inner<T> {
    val: T,
}

struct Middle<T> {
    inner: Inner<T>,
}

struct Outer<T> {
    middle: Middle<T>,
}

fn extract<T>(o: Outer<T>) -> i32 {
    return 999;
}

fn main() -> i32 {
    dec obj = Outer<i32> {
        middle: Middle<i32> {
            inner: Inner<i32> {
                val: 42,
            },
        },
    };
    dec res = extract<i32>(obj);
    if res != 999 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("deeply_nested_generic_substitutions", src);
    assert_eq!(code, 0, "Deeply nested generic substitutions test failed (code: {}, stderr: {})", code, stderr);
}

/// 6. Non-Generic Module Disambiguation: foo::compute vs bar::compute
#[test]
fn test_c_gap_02_non_generic_module_disambiguation() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;

module alpha {
    export fn compute() -> i32 {
        return 1000;
    }
}

module beta {
    export fn compute() -> i32 {
        return 2000;
    }
}

fn main() -> i32 {
    dec a = alpha::compute();
    dec b = beta::compute();
    if a + b != 3000 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("non_generic_module_disambiguation", src);
    assert_eq!(code, 0, "Non-generic module disambiguation failed (code: {}, stderr: {})", code, stderr);
}

/// 7. Drop Glue Canonical Mangling with Multi-Module Types
#[test]
fn test_c_gap_02_drop_glue_canonical_mangling() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <iter_adapters>;
import <iter_consumers>;
import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

module mod_a {
    export struct TrackA {
        export counter: *rw i32,
    }

    impl Drop for TrackA {
        fn drop(self: &rw Self) {
            unsafe {
                *self.counter = *self.counter + 10;
            }
        }
    }
}

module mod_b {
    export struct TrackB {
        export counter: *rw i32,
    }

    impl Drop for TrackB {
        fn drop(self: &rw Self) {
            unsafe {
                *self.counter = *self.counter + 20;
            }
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    {
        dec a = std::box_new<mod_a::TrackA>(mod_a::TrackA { counter: &rw count as *rw i32 });
        dec b = std::box_new<mod_b::TrackB>(mod_b::TrackB { counter: &rw count as *rw i32 });
    }
    if count != 30 {
        return 101;
    }
    return 0;
}
"#;
    let (code, _stdout, stderr) = compile_and_run("drop_glue_canonical_mangling", src);
    assert_eq!(code, 0, "Drop glue canonical mangling test failed (code: {}, stderr: {})", code, stderr);
}
