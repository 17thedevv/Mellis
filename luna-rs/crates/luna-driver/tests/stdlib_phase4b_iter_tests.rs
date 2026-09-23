use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_p4b_iter_{}_{}_{}",
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

fn run_binary(dir: &Path, src: &str, opts: &CompilerOptions) -> Result<(i32, String, String), String> {
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

#[test]
fn test_iter_chain() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("chain");

    let src = r#"
        import <vec>;
        import <iter_adapters>;

        fn main() -> i32 {
            // 1. Normal chain: [1, 2] + [3, 4]
            dec rw v1 = vec_new<i32>();
            v1.push(1);
            v1.push(2);

            dec rw v2 = vec_new<i32>();
            v2.push(3);
            v2.push(4);

            dec rw ch1 = iter_chain(v1.into_iter(), v2.into_iter());
            match ch1.next() {
                Option::Some(x) -> { if x != 1 { return 1; } },
                Option::None -> { return 2; },
            }
            match ch1.next() {
                Option::Some(x) -> { if x != 2 { return 3; } },
                Option::None -> { return 4; },
            }
            match ch1.next() {
                Option::Some(x) -> { if x != 3 { return 5; } },
                Option::None -> { return 6; },
            }
            match ch1.next() {
                Option::Some(x) -> { if x != 4 { return 7; } },
                Option::None -> { return 8; },
            }
            match ch1.next() {
                Option::Some(_) -> { return 9; },
                Option::None -> {},
            }

            // 2. Empty first: [] + [10, 20]
            dec rw v_empty1 = vec_new<i32>();
            dec rw v3 = vec_new<i32>();
            v3.push(10);
            v3.push(20);

            dec rw ch2 = iter_chain(v_empty1.into_iter(), v3.into_iter());
            match ch2.next() {
                Option::Some(x) -> { if x != 10 { return 10; } },
                Option::None -> { return 11; },
            }
            match ch2.next() {
                Option::Some(x) -> { if x != 20 { return 12; } },
                Option::None -> { return 13; },
            }
            match ch2.next() {
                Option::Some(_) -> { return 14; },
                Option::None -> {},
            }

            // 3. Empty second: [99] + []
            dec rw v4 = vec_new<i32>();
            v4.push(99);
            dec rw v_empty2 = vec_new<i32>();

            dec rw ch3 = iter_chain(v4.into_iter(), v_empty2.into_iter());
            match ch3.next() {
                Option::Some(x) -> { if x != 99 { return 15; } },
                Option::None -> { return 16; },
            }
            match ch3.next() {
                Option::Some(_) -> { return 17; },
                Option::None -> {},
            }

            // 4. Both empty: [] + []
            dec rw v_empty3 = vec_new<i32>();
            dec rw v_empty4 = vec_new<i32>();
            dec rw ch4 = iter_chain(v_empty3.into_iter(), v_empty4.into_iter());
            match ch4.next() {
                Option::Some(_) -> { return 18; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_iter_chain failed with code {}: {}", code, stderr);
}

#[test]
fn test_iter_position() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("position");

    let src = r#"
        import <vec>;
        import <iter_consumers>;

        fn is_forty_two(x: i32) -> bool {
            return x == 42;
        }

        fn is_missing(x: i32) -> bool {
            return x == 999;
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            v.push(42);
            v.push(50);

            dec pos = iter_position(v.into_iter(), is_forty_two);
            match pos {
                Option::Some(idx) -> {
                    if idx != (2 as u64) { return 1; }
                },
                Option::None -> { return 2; },
            }

            dec rw v2 = vec_new<i32>();
            v2.push(10);
            v2.push(20);

            dec pos2 = iter_position(v2.into_iter(), is_missing);
            match pos2 {
                Option::Some(_) -> { return 3; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_iter_position failed with code {}: {}", code, stderr);
}

#[test]
fn test_iter_nth() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("nth");

    let src = r#"
        import <vec>;
        import <iter_consumers>;

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(100);
            v.push(200);
            v.push(300);

            // nth(0) is first
            dec item0 = iter_nth(v.into_iter(), 0 as u64);
            match item0 {
                Option::Some(x) -> { if x != 100 { return 1; } },
                Option::None -> { return 2; },
            }

            // nth(2) is third
            dec rw v2 = vec_new<i32>();
            v2.push(100);
            v2.push(200);
            v2.push(300);
            dec item2 = iter_nth(v2.into_iter(), 2 as u64);
            match item2 {
                Option::Some(x) -> { if x != 300 { return 3; } },
                Option::None -> { return 4; },
            }

            // nth(5) is out of bounds -> None
            dec rw v3 = vec_new<i32>();
            v3.push(100);
            v3.push(200);
            dec item5 = iter_nth(v3.into_iter(), 5 as u64);
            match item5 {
                Option::Some(_) -> { return 5; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_iter_nth failed with code {}: {}", code, stderr);
}

#[test]
fn test_iter_last() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("last");

    let src = r#"
        import <vec>;
        import <iter_consumers>;

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(1);
            v.push(5);
            v.push(9);

            dec last_val = iter_last(v.into_iter());
            match last_val {
                Option::Some(x) -> { if x != 9 { return 1; } },
                Option::None -> { return 2; },
            }

            // Empty iterator -> None
            dec rw v_empty = vec_new<i32>();
            dec empty_last = iter_last(v_empty.into_iter());
            match empty_last {
                Option::Some(_) -> { return 3; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_iter_last failed with code {}: {}", code, stderr);
}

#[test]
fn test_iter_sum_i32() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("sum");

    let src = r#"
        import <vec>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            v.push(30);

            dec total = iter_sum_i32(v.into_iter());
            if total != 60 { return 1; }

            // Empty sum -> 0
            dec rw v_empty = vec_new<i32>();
            dec empty_total = iter_sum_i32(v_empty.into_iter());
            if empty_total != 0 { return 2; }

            // Chained iterator sum
            dec rw a = vec_new<i32>();
            a.push(1);
            a.push(2);

            dec rw b = vec_new<i32>();
            b.push(3);
            b.push(4);

            dec chained_total = iter_sum_i32(iter_chain(a.into_iter(), b.into_iter()));
            if chained_total != 10 { return 3; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_iter_sum_i32 failed with code {}: {}", code, stderr);
}
