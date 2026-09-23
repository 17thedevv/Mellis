use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_p4b_slice_{}_{}_{}",
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
fn test_slice_len_and_get() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("len_and_get");

    let src = r#"
        import <vec>;
        import <slice>;

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            v.push(30);

            // 1. slice_len
            if slice_len<i32>(v.as_slice()) != (3 as u64) { return 1; }

            // 2. slice_get within bounds
            match slice_get<i32>(v.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 10 { return 2; } },
                Option::None -> { return 3; },
            }
            match slice_get<i32>(v.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 30 { return 4; } },
                Option::None -> { return 5; },
            }

            // 3. slice_get out of bounds
            match slice_get<i32>(v.as_slice(), 3 as u64) {
                Option::Some(_) -> { return 6; },
                Option::None -> {},
            }

            // 4. slice_get_mut and mutate in-place
            match slice_get_mut<i32>(v.as_mut_slice(), 1 as u64) {
                Option::Some(x) -> { *x = 99; },
                Option::None -> { return 7; },
            }
            match slice_get<i32>(v.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 99 { return 8; } },
                Option::None -> { return 9; },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_slice_len_and_get failed with code {}: {}", code, stderr);
}

#[test]
fn test_slice_reverse() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("reverse");

    let src = r#"
        import <vec>;
        import <slice>;

        fn main() -> i32 {
            // Odd length: [1, 2, 3, 4, 5] -> [5, 4, 3, 2, 1]
            dec rw v1 = vec_new<i32>();
            v1.push(1);
            v1.push(2);
            v1.push(3);
            v1.push(4);
            v1.push(5);

            slice_reverse<i32>(v1.as_mut_slice());
            match slice_get<i32>(v1.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 5 { return 1; } },
                Option::None -> { return 2; },
            }
            match slice_get<i32>(v1.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 4 { return 3; } },
                Option::None -> { return 4; },
            }
            match slice_get<i32>(v1.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 3 { return 5; } },
                Option::None -> { return 6; },
            }
            match slice_get<i32>(v1.as_slice(), 3 as u64) {
                Option::Some(x) -> { if *x != 2 { return 7; } },
                Option::None -> { return 8; },
            }
            match slice_get<i32>(v1.as_slice(), 4 as u64) {
                Option::Some(x) -> { if *x != 1 { return 9; } },
                Option::None -> { return 10; },
            }

            // Even length: [10, 20] -> [20, 10]
            dec rw v2 = vec_new<i32>();
            v2.push(10);
            v2.push(20);

            slice_reverse<i32>(v2.as_mut_slice());
            match slice_get<i32>(v2.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 20 { return 11; } },
                Option::None -> { return 12; },
            }
            match slice_get<i32>(v2.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 10 { return 13; } },
                Option::None -> { return 14; },
            }

            // Single element: [42] -> [42]
            dec rw v3 = vec_new<i32>();
            v3.push(42);
            slice_reverse<i32>(v3.as_mut_slice());
            match slice_get<i32>(v3.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 42 { return 15; } },
                Option::None -> { return 16; },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_slice_reverse failed with code {}: {}", code, stderr);
}

#[test]
fn test_slice_contains_and_binary_search() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("search");

    let src = r#"
        import <vec>;
        import <slice>;

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            v.push(30);
            v.push(40);
            v.push(50);

            // 1. slice_contains_i32
            if slice_contains_i32(v.as_slice(), 30) == false { return 1; }
            if slice_contains_i32(v.as_slice(), 10) == false { return 2; }
            if slice_contains_i32(v.as_slice(), 50) == false { return 3; }
            if slice_contains_i32(v.as_slice(), 99) { return 4; }

            // 2. slice_binary_search_i32
            match slice_binary_search_i32(v.as_slice(), 10) {
                Option::Some(idx) -> { if idx != (0 as u64) { return 5; } },
                Option::None -> { return 6; },
            }
            match slice_binary_search_i32(v.as_slice(), 30) {
                Option::Some(idx) -> { if idx != (2 as u64) { return 7; } },
                Option::None -> { return 8; },
            }
            match slice_binary_search_i32(v.as_slice(), 50) {
                Option::Some(idx) -> { if idx != (4 as u64) { return 9; } },
                Option::None -> { return 10; },
            }
            match slice_binary_search_i32(v.as_slice(), 25) {
                Option::Some(_) -> { return 11; },
                Option::None -> {},
            }

            // 3. Binary search on empty slice
            dec rw v_empty = vec_new<i32>();
            match slice_binary_search_i32(v_empty.as_slice(), 10) {
                Option::Some(_) -> { return 12; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_slice_contains_and_binary_search failed with code {}: {}", code, stderr);
}

#[test]
fn test_slice_sort() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("sort");

    let src = r#"
        import <vec>;
        import <slice>;

        fn cmp_desc(a: &i32, b: &i32) -> i32 {
            if *a > *b { return 0 - 1; }
            if *a < *b { return 1; }
            return 0;
        }

        fn main() -> i32 {
            // 1. Small slice sort_i32 (<= 16 elements): [5, 2, 8, 1, 9, 3] -> [1, 2, 3, 5, 8, 9]
            dec rw v1 = vec_new<i32>();
            v1.push(5);
            v1.push(2);
            v1.push(8);
            v1.push(1);
            v1.push(9);
            v1.push(3);

            slice_sort_i32(v1.as_mut_slice());
            match slice_get<i32>(v1.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 1 { return 1; } },
                Option::None -> { return 2; },
            }
            match slice_get<i32>(v1.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 2 { return 3; } },
                Option::None -> { return 4; },
            }
            match slice_get<i32>(v1.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 3 { return 5; } },
                Option::None -> { return 6; },
            }
            match slice_get<i32>(v1.as_slice(), 3 as u64) {
                Option::Some(x) -> { if *x != 5 { return 7; } },
                Option::None -> { return 8; },
            }
            match slice_get<i32>(v1.as_slice(), 4 as u64) {
                Option::Some(x) -> { if *x != 8 { return 9; } },
                Option::None -> { return 10; },
            }
            match slice_get<i32>(v1.as_slice(), 5 as u64) {
                Option::Some(x) -> { if *x != 9 { return 11; } },
                Option::None -> { return 12; },
            }

            // 2. Larger slice (> 16 elements to exercise introsort partition & recursion)
            dec rw v2 = vec_new<i32>();
            dec rw cur: i32 = 20;
            while cur > 0 {
                v2.push(cur);
                cur = cur - 1;
            }
            // v2 is [20, 19, 18, ..., 1] of len 20
            slice_sort_i32(v2.as_mut_slice());
            dec rw check_idx: u64 = 0 as u64;
            while check_idx < (20 as u64) {
                match slice_get<i32>(v2.as_slice(), check_idx) {
                    Option::Some(x) -> {
                        dec expected = (check_idx + (1 as u64)) as i32;
                        if *x != expected { return 13; }
                    },
                    Option::None -> { return 14; },
                }
                check_idx = check_idx + (1 as u64);
            }

            // 3. Custom descending sort with slice_sort_by
            dec rw v3 = vec_new<i32>();
            v3.push(10);
            v3.push(50);
            v3.push(30);
            slice_sort_by<i32>(v3.as_mut_slice(), cmp_desc);
            match slice_get<i32>(v3.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 50 { return 15; } },
                Option::None -> { return 16; },
            }
            match slice_get<i32>(v3.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 30 { return 17; } },
                Option::None -> { return 18; },
            }
            match slice_get<i32>(v3.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 10 { return 19; } },
                Option::None -> { return 20; },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_slice_sort failed with code {}: {}", code, stderr);
}

#[test]
fn test_slice_copy_and_fill() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("copy_fill");

    let src = r#"
        import <vec>;
        import <slice>;

        fn main() -> i32 {
            // 1. slice_fill_i32
            dec rw v = vec_new<i32>();
            v.push(1);
            v.push(2);
            v.push(3);

            slice_fill_i32(v.as_mut_slice(), 77);
            match slice_get<i32>(v.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 77 { return 1; } },
                Option::None -> { return 2; },
            }
            match slice_get<i32>(v.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 77 { return 3; } },
                Option::None -> { return 4; },
            }
            match slice_get<i32>(v.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 77 { return 5; } },
                Option::None -> { return 6; },
            }

            // 2. slice_copy_i32
            dec rw src_vec = vec_new<i32>();
            src_vec.push(100);
            src_vec.push(200);
            src_vec.push(300);

            dec rw dst_vec = vec_new<i32>();
            dst_vec.push(0);
            dst_vec.push(0);
            dst_vec.push(0);

            slice_copy_i32(dst_vec.as_mut_slice(), src_vec.as_slice());
            match slice_get<i32>(dst_vec.as_slice(), 0 as u64) {
                Option::Some(x) -> { if *x != 100 { return 7; } },
                Option::None -> { return 8; },
            }
            match slice_get<i32>(dst_vec.as_slice(), 1 as u64) {
                Option::Some(x) -> { if *x != 200 { return 9; } },
                Option::None -> { return 10; },
            }
            match slice_get<i32>(dst_vec.as_slice(), 2 as u64) {
                Option::Some(x) -> { if *x != 300 { return 11; } },
                Option::None -> { return 12; },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_slice_copy_and_fill failed with code {}: {}", code, stderr);
}
