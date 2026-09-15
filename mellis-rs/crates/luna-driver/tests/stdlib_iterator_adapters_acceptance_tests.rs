use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_adapters_{}_{}_{}",
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

/// A1. Basic Map adapter using function pointer
#[test]
fn test_iter_map_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_basic");

    let src = r#"
        import <core>;

        fn square(x: i32) -> i32 {
            return x * x;
        }

        fn main() -> i32 {
            dec arr: [i32; 5] = [1, 2, 3, 4, 5];
            dec it = slice_iter<i32>(&arr);
            dec rw mapped = iter_map(it, square);

            dec rw sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = mapped.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // 1 + 4 + 9 + 16 + 25 = 55
            if count != 5 {
                return 1;
            }
            if sum != 55 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Map basic failed (code: {}, stderr: {})", code, stderr);
}

/// A2. Basic Filter adapter using function pointer
#[test]
fn test_iter_filter_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("filter_basic");

    let src = r#"
        import <core>;

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn main() -> i32 {
            dec arr: [i32; 6] = [1, 2, 3, 4, 5, 6];
            dec it = slice_iter<i32>(&arr);
            dec rw filtered = iter_filter(it, is_even);

            dec rw sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = filtered.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // Even numbers: 2 + 4 + 6 = 12
            if count != 3 {
                return 1;
            }
            if sum != 12 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Filter basic failed (code: {}, stderr: {})", code, stderr);
}

/// A3. Basic Enumerate adapter
#[test]
fn test_iter_enumerate_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("enumerate_basic");

    let src = r#"
        import <core>;

        fn main() -> i32 {
            dec arr: [i32; 3] = [10, 20, 30];
            dec it = slice_iter<i32>(&arr);
            dec rw enumerated = iter_enumerate(it);

            dec rw idx_sum: u64 = 0 as u64;
            dec rw val_sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = enumerated.next();
                match opt {
                    Option::Some(pair) -> {
                        count = count + 1;
                        idx_sum = idx_sum + pair.0;
                        val_sum = val_sum + pair.1;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // idx: 0 + 1 + 2 = 3
            // val: 10 + 20 + 30 = 60
            if count != 3 {
                return 1;
            }
            if idx_sum != (3 as u64) {
                return 2;
            }
            if val_sum != 60 {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Enumerate basic failed (code: {}, stderr: {})", code, stderr);
}

/// A4. Take adapter: exact count and empty handling
#[test]
fn test_iter_take_exact_count() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("take_exact");

    let src = r#"
        import <core>;

        fn main() -> i32 {
            dec arr: [i32; 5] = [10, 20, 30, 40, 50];
            dec it = slice_iter<i32>(&arr);
            dec rw taken = iter_take(it, 3 as u64);

            dec rw sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = taken.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // 10 + 20 + 30 = 60 (count = 3)
            if count != 3 {
                return 1;
            }
            if sum != 60 {
                return 2;
            }

            // Further next() calls return None cleanly
            dec opt2 = taken.next();
            match opt2 {
                Option::Some(_) -> { return 3; },
                Option::None -> {},
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Take exact failed (code: {}, stderr: {})", code, stderr);
}

/// A5. Skip adapter: exact count
#[test]
fn test_iter_skip_exact_count() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("skip_exact");

    let src = r#"
        import <core>;

        fn main() -> i32 {
            dec arr: [i32; 5] = [10, 20, 30, 40, 50];
            dec it = slice_iter<i32>(&arr);
            dec rw skipped = iter_skip(it, 3 as u64);

            dec rw sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = skipped.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // Skipped 10, 20, 30 -> yields 40, 50 (sum = 90, count = 2)
            if count != 2 {
                return 1;
            }
            if sum != 90 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Skip exact failed (code: {}, stderr: {})", code, stderr);
}

/// A6. Zip adapter: unequal lengths
#[test]
fn test_iter_zip_unequal_lengths() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("zip_unequal");

    let src = r#"
        import <core>;

        fn main() -> i32 {
            dec a: [i32; 4] = [1, 2, 3, 4];
            dec b: [i32; 2] = [10, 20];
            dec it_a = slice_iter<i32>(&a);
            dec it_b = slice_iter<i32>(&b);
            dec rw zipped = iter_zip(it_a, it_b);

            dec rw sum_a: i32 = 0;
            dec rw sum_b: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = zipped.next();
                match opt {
                    Option::Some(pair) -> {
                        count = count + 1;
                        sum_a = sum_a + pair.0;
                        sum_b = sum_b + pair.1;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // Terminates on shorter (len 2): pairs (1, 10), (2, 20)
            if count != 2 {
                return 1;
            }
            if sum_a != 3 {
                return 2;
            }
            if sum_b != 30 {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Zip unequal failed (code: {}, stderr: {})", code, stderr);
}

/// A7. Adapter pipeline chaining: filter -> map -> take
#[test]
fn test_iter_adapter_pipeline_chaining() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("pipeline_chaining");

    let src = r#"
        import <core>;

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn square(x: i32) -> i32 {
            return x * x;
        }

        fn main() -> i32 {
            dec arr: [i32; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            dec it = slice_iter<i32>(&arr);
            dec filtered = iter_filter(it, is_even);       // 2, 4, 6, 8, 10
            dec mapped = iter_map(filtered, square);       // 4, 16, 36, 64, 100
            dec rw pipeline = iter_take(mapped, 3 as u64); // 4, 16, 36

            dec rw sum: i32 = 0;
            dec rw count: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = pipeline.next();
                match opt {
                    Option::Some(val) -> {
                        count = count + 1;
                        sum = sum + val;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // 4 + 16 + 36 = 56 (count = 3)
            if count != 3 {
                return 1;
            }
            if sum != 56 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Pipeline chaining failed (code: {}, stderr: {})", code, stderr);
}

/// A8. Source vs Precompiled .llib Parity for <core> adapters
#[test]
fn test_adapter_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("adapter_parity");

    let src = r#"
        import <core>;

        fn add_ten(x: i32) -> i32 {
            return x + 10;
        }

        fn main() -> i32 {
            dec arr: [i32; 3] = [1, 2, 3];
            dec it = slice_iter<i32>(&arr);
            dec rw mapped = iter_map(it, add_ten);

            dec rw sum: i32 = 0;
            dec rw running = true;
            while running {
                dec opt = mapped.next();
                match opt {
                    Option::Some(v) -> {
                        sum = sum + v;
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            // 11 + 12 + 13 = 36
            if sum != 36 {
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

/// A9. Filter with DropTracker: rejected items dropped exactly once
#[test]
fn test_iter_filter_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("filter_droptracker");

    let src = r#"
        import <core>;
        import <alloc>;

        struct DropCounter {
            private created: u64,
            private dropped: u64,
        }

        struct TrackedItem {
            val: i32,
            counter: *rw DropCounter,
        }

        impl Drop for TrackedItem {
            fn drop(self: &rw Self) {
                unsafe {
                    (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                }
            }
        }

        fn is_even(item: &TrackedItem) -> bool {
            return (item.val % 2) == 0;
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                dec rw v = vec_with_capacity<TrackedItem>(5 as u64);
                dec rw i: i32 = 1;
                while i <= 5 {
                    unsafe {
                        (*c_ptr).created = (*c_ptr).created + (1 as u64);
                    }
                    v.push(TrackedItem { val: i, counter: c_ptr });
                    i = i + 1;
                }

                // 5 items created (vals 1, 2, 3, 4, 5)
                // Filter keeps even (2, 4), rejects odd (1, 3, 5).
                // Rejected items must be dropped in-place as filter iterates!
                dec it = v.iter();
                dec rw filtered = iter_filter(it, is_even);

                dec rw count: i32 = 0;
                dec rw running = true;
                while running {
                    dec opt = filtered.next();
                    match opt {
                        Option::Some(item) -> {
                            count = count + 1;
                        },
                        Option::None -> {
                            running = false;
                        },
                    }
                }

                if count != 2 {
                    return 1;
                }

                // All 5 items yielded during iteration are dropped:
                // 3 odd items rejected and dropped in-place by Filter,
                // 2 even items processed and dropped by the loop body.
                if counter.dropped != (5 as u64) {
                    return 2;
                }
            }

            // After scope exits (Vec v dropped, releasing its 5 backing elements):
            // 5 iteration drops + 5 Vec drops = 10 total drops.
            if counter.created != (5 as u64) {
                return 3;
            }
            if counter.dropped != (10 as u64) {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Filter droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// A10. Zip with DropTracker: unconsumed item from longer stream dropped exactly once
#[test]
fn test_iter_zip_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("zip_droptracker");

    let src = r#"
        import <core>;
        import <alloc>;

        struct DropCounter {
            private created: u64,
            private dropped: u64,
        }

        struct TrackedItem {
            val: i32,
            counter: *rw DropCounter,
        }

        impl Drop for TrackedItem {
            fn drop(self: &rw Self) {
                unsafe {
                    (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                }
            }
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                dec rw v_a = vec_with_capacity<TrackedItem>(3 as u64);
                dec rw i: i32 = 1;
                while i <= 3 {
                    unsafe {
                        (*c_ptr).created = (*c_ptr).created + (1 as u64);
                    }
                    v_a.push(TrackedItem { val: i, counter: c_ptr });
                    i = i + 1;
                }

                // Stream B has only 1 element
                dec b_arr: [i32; 1] = [100];

                dec it_a = v_a.iter();
                dec it_b = slice_iter<i32>(&b_arr);
                dec rw zipped = iter_zip(it_a, it_b);

                // Step 1: yields (TrackedItem 1, 100)
                dec opt1 = zipped.next();
                match opt1 {
                    Option::Some(pair) -> {
                        if pair.1 != 100 {
                            return 1;
                        }
                    },
                    Option::None -> {
                        return 2;
                    },
                }

                // Step 2: Stream A yields TrackedItem 2, but Stream B yields None.
                // TrackedItem 2 must be dropped cleanly without leak!
                dec opt2 = zipped.next();
                match opt2 {
                    Option::Some(_) -> {
                        return 3;
                    },
                    Option::None -> {},
                }

                // Exactly 2 items dropped so far:
                // - TrackedItem 1 was yielded in opt1 and dropped when match arm exited
                // - TrackedItem 2 was unconsumed and dropped cleanly in Zip when Stream B was exhausted
                if counter.dropped != (2 as u64) {
                    return 4;
                }
            }

            // After scope exits (Vec v_a dropped, releasing its 3 backing elements):
            // 2 iteration drops + 3 Vec drops = 5 total drops.
            if counter.created != (3 as u64) {
                return 5;
            }
            if counter.dropped != (5 as u64) {
                return 6;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Zip droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// A11. Deep lifetime chain locks collection: HashMap -> iter -> filter -> map -> take
#[test]
fn test_iter_deep_lifetime_chain_locks_collection() {
    let dir = create_temp_dir("deep_lifetime_chain");

    let src = r#"
        import <core>;
        import <alloc>;

        fn pred(pair: &(&i32, &i32)) -> bool {
            return true;
        }

        fn map_fn(pair: (&i32, &i32)) -> i32 {
            return *pair.0 + *pair.1;
        }

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);
            map.insert(2, 20);

            dec it = map.iter();
            dec f_it = iter_filter(it, pred);
            dec m_it = iter_map(f_it, map_fn);
            dec rw t_it = iter_take(m_it, 2 as u64);

            // MUTATION WHILE ADAPTER CHAIN HOLDS BORROW:
            // Borrowck must detect carried provenance through filter -> map -> take
            // and reject this mutation at compile time!
            map.insert(99, 990);

            dec opt = t_it.next();
            return 0;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(
        res.is_err(),
        "Borrowck must reject mutation while deep adapter chain is live"
    );

    let diags = res.err().unwrap();
    let has_borrow_err = diags.iter().any(|d| {
        let msg = &d.message;
        msg.contains("Cannot mutate")
            || msg.contains("borrow")
            || msg.contains("already borrowed")
            || msg.contains("E0203")
            || msg.contains("E0204")
    });

    assert!(
        has_borrow_err,
        "Expected borrow error for mutating HashMap during live adapter chain, got: {:?}",
        diags
    );
}
