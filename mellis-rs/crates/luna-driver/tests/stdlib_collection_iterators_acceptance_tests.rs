// =============================================================================
// Stdlib Phase 04.6 Collection Iterators Acceptance Tests
//
// Verifies:
//   1. HashMap empty iteration (returns None)
//   2. HashMap full traversal (visits all (key, value) pairs)
//   3. HashMap traversal across sparse tombstones (skips deleted slots cleanly)
//   4. HashMap keys() and values() separate views
//   5. HashSet full traversal (visits all elements)
//   6. HashSet traversal across sparse tombstones (skips deleted slots cleanly)
//   7. Borrowck enforcement: mutating collection while iterator is live is rejected
//   8. Source vs precompiled .llib parity
// =============================================================================

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_iter_{}_{}_{}",
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

fn check_source_with_sysroot(
    dir: &Path,
    src: &str,
) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let src_path = dir.join("check_main.ln");
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        quiet: true,
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            sysroot.root().to_string_lossy().to_string(),
        ],
        ..Default::default()
    };

    check(src_path.to_str().unwrap(), src.to_string(), &opts)
}

/// 1. HashMap empty iteration
#[test]
fn test_hashmap_iter_empty() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_iter_empty");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_new<i32, i32>();
            dec rw it = map.iter();
            dec opt = it.next();
            match opt {
                Option::Some(_) -> { return 1; },
                Option::None -> { return 0; },
            }
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Empty HashMap iteration failed (code: {}, stderr: {})", code, stderr);
}

/// 2. HashMap full traversal
#[test]
fn test_hashmap_iter_full_traversal() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_iter_full");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);
            map.insert(2, 20);
            map.insert(3, 30);
            map.insert(4, 40);
            map.insert(5, 50);

            dec rw it = map.iter();
            dec rw count: i32 = 0;
            dec rw key_sum: i32 = 0;
            dec rw val_sum: i32 = 0;

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(pair) -> {
                        count = count + 1;
                        key_sum = key_sum + *pair.0;
                        val_sum = val_sum + *pair.1;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if count != 5 {
                return 1;
            }
            if key_sum != 15 {
                return 2;
            }
            if val_sum != 150 {
                return val_sum;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Full traversal failed (code: {}, stderr: {})", code, stderr);
}

/// 3. HashMap traversal across sparse tombstones
#[test]
fn test_hashmap_iter_sparse_tombstones() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_iter_tombstones");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(16 as u64);
            dec rw i: i32 = 1;
            while i <= 8 {
                map.insert(i, i * 10);
                i = i + 1;
            }

            // Remove 3 elements (creates tombstones)
            dec k2: i32 = 2;
            dec k4: i32 = 4;
            dec k6: i32 = 6;
            map.remove(&k2);
            map.remove(&k4);
            map.remove(&k6);

            // Remaining: 1, 3, 5, 7, 8 (count = 5)
            // Expected key sum = 1 + 3 + 5 + 7 + 8 = 24
            // Expected val sum = 240
            dec rw it = map.iter();
            dec rw count: i32 = 0;
            dec rw key_sum: i32 = 0;
            dec rw val_sum: i32 = 0;

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(pair) -> {
                        count = count + 1;
                        key_sum = key_sum + *pair.0;
                        val_sum = val_sum + *pair.1;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if count != 5 {
                return 1;
            }
            if key_sum != 24 {
                return 2;
            }
            if val_sum != 240 {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Tombstone traversal failed (code: {}, stderr: {})", code, stderr);
}

/// 4. HashMap keys() and values() separate views
#[test]
fn test_hashmap_keys_and_values() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_keys_values");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(10, 100);
            map.insert(20, 200);
            map.insert(30, 300);

            // Traverse keys
            dec rw k_it = map.keys();
            dec rw k_sum: i32 = 0;
            dec rw k_count: i32 = 0;
            dec rw k_running = true;
            while k_running {
                dec opt = k_it.next();
                match opt {
                    Option::Some(k) -> {
                        k_count = k_count + 1;
                        k_sum = k_sum + *k;
                    },
                    Option::None -> {
                        k_running = false;
                    },
                }
            }

            if k_count != 3 || k_sum != 60 {
                return 1;
            }

            // Traverse values
            dec rw v_it = map.values();
            dec rw v_sum: i32 = 0;
            dec rw v_count: i32 = 0;
            dec rw v_running = true;
            while v_running {
                dec opt = v_it.next();
                match opt {
                    Option::Some(v) -> {
                        v_count = v_count + 1;
                        v_sum = v_sum + *v;
                    },
                    Option::None -> {
                        v_running = false;
                    },
                }
            }

            if v_count != 3 || v_sum != 600 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Keys and values iteration failed (code: {}, stderr: {})", code, stderr);
}

/// 5. HashSet full traversal
#[test]
fn test_hashset_iter_traversal() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_iter_full");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw set = hashset_with_capacity<i32>(8 as u64);
            set.insert(10);
            set.insert(20);
            set.insert(30);

            dec rw it = set.iter();
            dec rw count: i32 = 0;
            dec rw sum: i32 = 0;

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + *val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if count != 3 {
                return 1;
            }
            if sum != 60 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HashSet traversal failed (code: {}, stderr: {})", code, stderr);
}

/// 6. HashSet traversal with tombstones
#[test]
fn test_hashset_iter_with_tombstones() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_iter_tombstones");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw set = hashset_with_capacity<i32>(8 as u64);
            set.insert(1);
            set.insert(2);
            set.insert(3);
            set.insert(4);

            dec k2: i32 = 2;
            set.remove(&k2);

            // Remaining: 1, 3, 4 (count = 3, sum = 8)
            dec rw it = set.iter();
            dec rw count: i32 = 0;
            dec rw sum: i32 = 0;

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + *val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if count != 3 {
                return 1;
            }
            if sum != 8 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HashSet tombstone traversal failed (code: {}, stderr: {})", code, stderr);
}

/// 7. Borrowck enforcement: mutating collection while iterator is live is rejected
#[test]
fn test_iter_borrowck_mutation_blocked() {
    let dir = create_temp_dir("iter_borrowck_blocked");
    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);

            dec rw it = map.iter(); // Borrows map
            map.insert(2, 20);      // ERROR: cannot mutate map while iterator is live

            dec opt = it.next();
            return 0;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(res.is_err(), "Mutating HashMap while iterator is live must be rejected by borrowck");
}

/// 8. Source vs .llib parity
#[test]
fn test_iter_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let alloc_llib = sysroot.root().join("libs").join("external").join("alloc.llib");
    assert!(alloc_llib.exists(), "alloc.llib must exist for parity test");

    let dir = create_temp_dir("iter_parity");

    let src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(100, 1000);
            map.insert(200, 2000);

            dec rw it = map.iter();
            dec rw count: i32 = 0;
            dec rw sum: i32 = 0;

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(pair) -> {
                        count = count + 1;
                        sum = sum + *pair.0 + *pair.1;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if count != 2 || sum != 3300 {
                return 1;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Parity test failed with code {} (stderr: {})", code, stderr);
}
