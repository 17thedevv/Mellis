use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_dispatch_{}_{}_{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

fn run_binary_with_output(
    dir: &Path,
    src: &str,
    opts: &CompilerOptions,
) -> Result<(i32, String, String), String> {
    let src_path = dir.join("main.ln");
    let exe_path = dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    fs::write(&src_path, src).map_err(|e| e.to_string())?;

    let mut compile_opts = opts.clone();
    compile_opts.output_path = Some(exe_path.to_str().unwrap().to_string());

    let compile_res = compile(src_path.to_str().unwrap(), src.to_string(), &compile_opts);
    if let Err(err) = compile_res {
        return Err(format!("Compilation failed: {:?}", err));
    }

    let output = Command::new(&exe_path)
        .output()
        .map_err(|e| format!("Execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((code, stdout, stderr))
}

/// 1. Primitive Trait Dispatch: Calling `fn hash_val<T: Hash>(x: T) -> u64` with primitives
#[test]
fn test_generic_trait_dispatch_hash_primitives() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("hash_primitives");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn hash_val<T: Hash>(x: T) -> u64 {
            return x.hash();
        }

        fn main() -> i32 {
            dec h_u64: u64 = hash_val<u64>(42 as u64);
            dec h_i32: u64 = hash_val<i32>(100 as i32);
            dec h_bool_t: u64 = hash_val<bool>(true);
            dec h_bool_f: u64 = hash_val<bool>(false);
            dec h_char: u64 = hash_val<char>('A');

            if h_u64 != (42 as u64) {
                return 1;
            }
            if h_i32 != (100 as u64) {
                return 2;
            }
            if h_bool_t != (1 as u64) {
                return 3;
            }
            if h_bool_f != (0 as u64) {
                return 4;
            }
            if h_char != (65 as u64) {
                return 5;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Binary returned exit code {} (stderr: {})", code, stderr);
}

/// 2. Custom Nominal Struct Dispatch: Calling `fn hash_val<T: Hash>` with user-defined struct
#[test]
fn test_generic_trait_dispatch_hash_custom_struct() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("hash_custom_struct");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct Point {
            x: u64,
            y: u64,
        }

        impl Hash for Point {
            fn hash(self: &Self) -> u64 {
                return self.x + self.y * (31 as u64);
            }
        }

        fn hash_val<T: Hash>(x: T) -> u64 {
            return x.hash();
        }

        fn main() -> i32 {
            dec pt = Point { x: 10 as u64, y: 20 as u64 };
            dec h = hash_val<Point>(pt);

            // 10 + 20 * 31 = 10 + 620 = 630
            if h != (630 as u64) {
                return 1;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Binary returned exit code {} (stderr: {})", code, stderr);
}

/// 3. Eq Trait Dispatch: Calling `fn check_eq<T: Eq>(a: &T, b: &T) -> bool` with primitives & custom struct
#[test]
fn test_generic_trait_dispatch_eq() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("eq_dispatch");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct Color {
            r: u8,
            g: u8,
            b: u8,
        }

        impl Eq for Color {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.r == other.r && self.g == other.g && self.b == other.b;
            }
        }

        fn check_eq<T: Eq>(a: &T, b: &T) -> bool {
            return a.eq(b);
        }

        fn main() -> i32 {
            dec x: i32 = 42;
            dec y: i32 = 42;
            dec z: i32 = 99;

            if check_eq<i32>(&x, &y) == false {
                return 1;
            }
            if check_eq<i32>(&x, &z) == true {
                return 2;
            }

            dec c1 = Color { r: 255 as u8, g: 0 as u8, b: 128 as u8 };
            dec c2 = Color { r: 255 as u8, g: 0 as u8, b: 128 as u8 };
            dec c3 = Color { r: 0 as u8, g: 255 as u8, b: 128 as u8 };

            if check_eq<Color>(&c1, &c2) == false {
                return 3;
            }
            if check_eq<Color>(&c1, &c3) == true {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Binary returned exit code {} (stderr: {})", code, stderr);
}

/// 4. Clone Trait Dispatch: Calling `fn duplicate<T: Clone>(x: &T) -> T`
#[test]
fn test_generic_trait_dispatch_clone() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("clone_dispatch");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct Vector2 {
            x: i32,
            y: i32,
        }

        impl Clone for Vector2 {
            fn clone(self: &Self) -> Self {
                return Vector2 { x: self.x, y: self.y };
            }
        }

        fn duplicate<T: Clone>(x: &T) -> T {
            return x.clone();
        }

        fn main() -> i32 {
            dec a: i32 = 1234;
            dec b = duplicate<i32>(&a);
            if b != 1234 {
                return 1;
            }

            dec v1 = Vector2 { x: 50, y: 100 };
            dec v2 = duplicate<Vector2>(&v1);
            if v2.x != 50 || v2.y != 100 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Binary returned exit code {} (stderr: {})", code, stderr);
}

/// 5. Negative Test: Calling `hash_val<T>` on a type that does NOT implement `Hash` must fail typechecking
#[test]
fn test_generic_trait_dispatch_negative_unhashable_rejected() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("negative_unhashable");
    let src_path = dir.join("main.ln");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct Unhashable {
            val: i32,
        }

        fn hash_val<T: Hash>(x: T) -> u64 {
            return x.hash();
        }

        fn main() -> i32 {
            dec u = Unhashable { val: 42 };
            dec h = hash_val<Unhashable>(u);
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let check_res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(check_res.is_err(), "Typechecking unhashable generic call must fail");
}

/// 6. Negative Test: `f64` does not implement `Eq` in v1 (Eq-1 invariant: NaN != NaN)
#[test]
fn test_generic_trait_dispatch_negative_f64_not_eq() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("negative_f64_eq");
    let src_path = dir.join("main.ln");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn check_eq<T: Eq>(a: &T, b: &T) -> bool {
            return a.eq(b);
        }

        fn main() -> i32 {
            dec a: f64 = 1.0;
            dec b: f64 = 2.0;
            dec eq = check_eq<f64>(&a, &b);
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let check_res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(check_res.is_err(), "Typechecking f64: Eq must fail because f64 does not implement Eq");
}

/// 7. Monomorphization Distinct Symbols: Calling `hash_val<u64>` and `hash_val<i32>` produces separate specialized instances
#[test]
fn test_generic_trait_dispatch_distinct_monomorphization() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("distinct_mono");

    let src = r#"
        import <core/panic>;
        import <hash>;
        import <cmp>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn hash_val<T: Hash>(x: T) -> u64 {
            return x.hash();
        }

        fn main() -> i32 {
            dec a = hash_val<u64>(10 as u64);
            dec b = hash_val<i32>(20 as i32);
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Compilation and execution must succeed");

    assert_eq!(code, 0, "Execution failed with stderr: {}", stderr);
}
