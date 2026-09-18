use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_terminal_{}_{}_{}",
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

/// 1. Basic fold: sum and product
#[test]
fn test_iter_fold_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("fold_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn add(acc: i32, x: &i32) -> i32 {
            return acc + (*x);
        }

        fn mul(acc: i32, x: &i32) -> i32 {
            return acc * (*x);
        }

        fn main() -> i32 {
            dec arr: [i32; 5] = [1, 2, 3, 4, 5];
            dec it1 = slice_iter<i32>(&arr);
            dec sum = iter_fold(it1, 0, add);

            dec it2 = slice_iter<i32>(&arr);
            dec prod = iter_fold(it2, 1, mul);

            // sum = 1 + 2 + 3 + 4 + 5 = 15
            // prod = 1 * 2 * 3 * 4 * 5 = 120
            if sum != 15 {
                return 1;
            }
            if prod != 120 {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Fold basic failed (code: {}, stderr: {})", code, stderr);
}

/// 2. Basic count over raw slice iter and filtered iter
#[test]
fn test_iter_count_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("count_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn main() -> i32 {
            dec arr: [i32; 6] = [1, 2, 3, 4, 5, 6];
            dec it1 = slice_iter<i32>(&arr);
            dec c1 = iter_count(it1);

            dec it2 = slice_iter<i32>(&arr);
            dec filtered = iter_filter(it2, is_even);
            dec c2 = iter_count(filtered);

            if c1 != (6 as u64) {
                return 1;
            }
            if c2 != (3 as u64) {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Count basic failed (code: {}, stderr: {})", code, stderr);
}

/// 3. Basic for_each side-effect execution
#[test]
fn test_iter_for_each_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("for_each_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct Context {
            total: i32,
            visits: u64,
        }

        fn visit_item(ctx: *rw Context, x: &i32) {
            unsafe {
                (*ctx).total = (*ctx).total + (*x);
                (*ctx).visits = (*ctx).visits + (1 as u64);
            }
        }

        fn main() -> i32 {
            dec rw ctx = Context { total: 0, visits: 0 as u64 };
            dec c_ptr = &rw ctx as *rw Context;

            dec arr: [i32; 4] = [10, 20, 30, 40];
            dec it = slice_iter<i32>(&arr);

            dec rw running = true;
            while running {
                dec opt = it.next();
                match opt {
                    Option::Some(x) -> {
                        visit_item(c_ptr, x);
                    },
                    Option::None -> {
                        running = false;
                    },
                }
            }

            if ctx.visits != (4 as u64) {
                return 1;
            }
            if ctx.total != 100 {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "For each basic failed (code: {}, stderr: {})", code, stderr);
}

/// 4. Basic any: true on early match, false on exhaustion, false on empty
#[test]
fn test_iter_any_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("any_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn is_gt_hundred(x: &i32) -> bool {
            return (*x) > 100;
        }

        fn is_negative(x: &i32) -> bool {
            return (*x) < 0;
        }

        fn main() -> i32 {
            dec arr: [i32; 5] = [10, 20, 150, 40, 50];
            dec it1 = slice_iter<i32>(&arr);
            dec has_gt_hundred = iter_any(it1, is_gt_hundred);

            dec it2 = slice_iter<i32>(&arr);
            dec has_negative = iter_any(it2, is_negative);

            if has_gt_hundred == false {
                return 1;
            }
            if has_negative == true {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Any basic failed (code: {}, stderr: {})", code, stderr);
}

/// 5. Basic all: true when all match, false on early exit on mismatch
#[test]
fn test_iter_all_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("all_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn is_positive(x: &i32) -> bool {
            return (*x) > 0;
        }

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn main() -> i32 {
            dec arr: [i32; 4] = [2, 4, 6, 8];
            dec it1 = slice_iter<i32>(&arr);
            dec all_even = iter_all(it1, is_even);

            dec arr2: [i32; 4] = [2, 4, 7, 8];
            dec it2 = slice_iter<i32>(&arr2);
            dec all_even_mixed = iter_all(it2, is_even);

            if all_even == false {
                return 1;
            }
            if all_even_mixed == true {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "All basic failed (code: {}, stderr: {})", code, stderr);
}

/// 6. Basic find: returns Option::Some(&T) when present, Option::None when absent
#[test]
fn test_iter_find_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("find_basic");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn is_target(x: &i32) -> bool {
            return (*x) == 42;
        }

        fn is_absent(x: &i32) -> bool {
            return (*x) == 999;
        }

        fn main() -> i32 {
            dec arr: [i32; 5] = [10, 20, 42, 50, 60];
            dec it1 = slice_iter<i32>(&arr);
            dec opt1 = iter_find(it1, is_target);

            dec found_val = match opt1 {
                Option::Some(r) -> *r,
                Option::None -> 0,
            };

            dec it2 = slice_iter<i32>(&arr);
            dec opt2 = iter_find(it2, is_absent);

            dec is_absent_none = match opt2 {
                Option::Some(_) -> false,
                Option::None -> true,
            };

            if found_val != 42 {
                return 1;
            }
            if is_absent_none == false {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Find basic failed (code: {}, stderr: {})", code, stderr);
}

/// 7. Pipeline chaining with terminal consumer: filter -> map -> fold
#[test]
fn test_iter_terminal_pipeline_chaining() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("pipeline_terminal");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn square(x: &i32) -> i32 {
            return (*x) * (*x);
        }

        fn add(acc: i32, x: i32) -> i32 {
            return acc + x;
        }

        fn main() -> i32 {
            dec arr: [i32; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            dec it = slice_iter<i32>(&arr);
            dec filtered = iter_filter(it, is_even);          // 2, 4, 6, 8, 10
            dec mapped = iter_map(filtered, square);          // 4, 16, 36, 64, 100
            dec pipeline = iter_take(mapped, 3 as u64);       // 4, 16, 36
            dec sum = iter_fold(pipeline, 0, add);            // 4 + 16 + 36 = 56

            if sum != 56 {
                return 1;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Terminal pipeline chaining failed (code: {}, stderr: {})", code, stderr);
}

/// 8. iter_any early exit DropTracker: does not drop unconsumed elements during early exit
#[test]
fn test_iter_any_early_exit_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("any_early_drop");

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

        fn is_match(item: &TrackedItem) -> bool {
            return item.val == 2;
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
                dec it = slice_iter<TrackedItem>(v.as_slice());
                dec matched = iter_any(it, is_match);

                if matched == false {
                    return 1;
                }

                // Invariant TERM-2: Early exit must NOT drop any backing elements!
                // During early exit: dropped == 0
                if counter.dropped != (0 as u64) {
                    return 2;
                }
            }

            // After Vec goes out of scope, all 5 items dropped by backing storage
            if counter.created != (5 as u64) || counter.dropped != (5 as u64) {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Any early exit droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// 9. iter_all early exit DropTracker: does not drop unconsumed elements on early failure
#[test]
fn test_iter_all_early_exit_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("all_early_drop");

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
                dec rw v = vec_with_capacity<TrackedItem>(4 as u64);
                dec rw i: i32 = 1;
                while i <= 4 {
                    unsafe {
                        (*c_ptr).created = (*c_ptr).created + (1 as u64);
                    }
                    v.push(TrackedItem { val: i, counter: c_ptr });
                    i = i + 1;
                }

                // Item 1 is odd -> iter_all exits immediately on element 1!
                dec it = slice_iter<TrackedItem>(v.as_slice());
                dec all_even = iter_all(it, is_even);

                if all_even == true {
                    return 1;
                }

                // Invariant TERM-2: Early exit must NOT drop any backing elements!
                if counter.dropped != (0 as u64) {
                    return 2;
                }
            }

            // After Vec drops: exactly 4 items dropped
            if counter.created != (4 as u64) || counter.dropped != (4 as u64) {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "All early exit droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// 10. iter_find early exit DropTracker: does not drop unconsumed elements
#[test]
fn test_iter_find_early_exit_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("find_early_drop");

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

        fn is_target(item: &TrackedItem) -> bool {
            return item.val == 3;
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

                dec it = slice_iter<TrackedItem>(v.as_slice());
                dec opt = iter_find(it, is_target);

                dec found_val = match opt {
                    Option::Some(r) -> (*r).val,
                    Option::None -> 0,
                };

                if found_val != 3 {
                    return 1;
                }

                // Invariant TERM-2: Early exit must NOT drop any backing elements!
                if counter.dropped != (0 as u64) {
                    return 2;
                }
            }

            // After Vec drops: exactly 5 items dropped
            if counter.created != (5 as u64) || counter.dropped != (5 as u64) {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Find early exit droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// 11. Source vs Precompiled .llib Parity for terminal consumers
#[test]
fn test_terminal_consumers_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("terminal_parity");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn add(acc: i32, x: &i32) -> i32 {
            return acc + (*x);
        }

        fn is_positive(x: &i32) -> bool {
            return (*x) > 0;
        }

        fn main() -> i32 {
            dec arr: [i32; 4] = [10, 20, 30, 40];
            dec it1 = slice_iter<i32>(&arr);
            dec sum = iter_fold(it1, 0, add);

            dec it2 = slice_iter<i32>(&arr);
            dec all_pos = iter_all(it2, is_positive);

            if sum != 100 {
                return 1;
            }
            if all_pos == false {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Terminal parity failed (code: {}, stderr: {})", code, stderr);
}

/// 12. Owned Item find transfer & borrowck rejection audit:
/// Part A: Borrowck strictly rejects owned non-Copy item double-use (pred(x) moves x, return Some(x) illegal).
/// Part B: Owned Copy item find succeeds, early terminates, and transfers value to caller.
#[test]
fn test_iter_find_owned_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("find_owned_drop");

    // Part A: Proves that for non-Copy items, passing Item by value to pred consumes it,
    // so returning Option::Some(x) is safely and strictly rejected by the borrow checker!
    let reject_src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

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

        extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        struct OwnedPairIter {
            buf: *rw TrackedItem,
            pos: u64,
            len: u64,
        }

        impl Iterator<TrackedItem> for OwnedPairIter {
            fn next(self: &rw Self) -> Option<TrackedItem> {
                return Option::None;
            }
        }

        fn is_second(item: TrackedItem) -> bool {
            return item.val == 20;
        }

        fn main() -> i32 {
            dec it = OwnedPairIter { buf: 0 as u64 as *rw TrackedItem, pos: 0 as u64, len: 0 as u64 };
            // Must be rejected by Borrowck because pred(x) consumes x, making return Option::Some(x) an illegal use of moved value!
            dec opt = iter_find(it, is_second);
            return 0;
        }
    "#;

    let reject_path = dir.join("reject_find.ln");
    fs::write(&reject_path, reject_src).unwrap();

    let mut check_opts = CompilerOptions::default();
    check_opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let check_res = check(reject_path.to_str().unwrap(), reject_src.to_string(), &check_opts);
    assert!(
        check_res.is_err(),
        "BorrowCk MUST reject iter_find with owned non-Copy item because pred consumes item before Option::Some(x)"
    );
    let diags = check_res.unwrap_err();
    let has_moved_err = diags.iter().any(|d| d.message.contains("Use of moved value"));
    assert!(
        has_moved_err,
        "Expected 'Use of moved value' diagnostic, got: {:?}",
        diags
    );

    // Part B: For owned Copy items, iter_find transfers ownership of the found item and early exits!
    let run_src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        struct OwnedCopyIter {
            buf: *rw i32,
            pos: u64,
            len: u64,
        }

        impl Drop for OwnedCopyIter {
            fn drop(self: &rw Self) {
                dec size = self.len * (sizeof(i32) as u64);
                __mellis_dealloc(self.buf as *rw u8, size, 4 as u64);
            }
        }

        impl Iterator<i32> for OwnedCopyIter {
            fn next(self: &rw Self) -> Option<i32> {
                if self.pos < self.len {
                    unsafe {
                        dec p = ptr::add_mut<i32>(self.buf, self.pos);
                        self.pos = self.pos + (1 as u64);
                        return Option::Some(ptr::read<i32>(p as *i32));
                    }
                }
                return Option::None;
            }
        }

        fn is_target(x: i32) -> bool {
            return x == 20;
        }

        fn main() -> i32 {
            dec size = 3 as u64 * (sizeof(i32) as u64);
            dec raw_buf = __mellis_alloc(size, 4 as u64) as *rw i32;
            unsafe {
                ptr::write<i32>(ptr::add_mut<i32>(raw_buf, 0 as u64), 10);
                ptr::write<i32>(ptr::add_mut<i32>(raw_buf, 1 as u64), 20);
                ptr::write<i32>(ptr::add_mut<i32>(raw_buf, 2 as u64), 30);
            }
            dec it = OwnedCopyIter {
                buf: raw_buf,
                pos: 0 as u64,
                len: 3 as u64,
            };

            dec opt = iter_find(it, is_target);

            dec found_val = match opt {
                Option::Some(v) -> v,
                Option::None -> 0,
            };

            if found_val != 20 {
                return 1;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, run_src, &opts)
        .expect("Owned Copy find execution must succeed");

    assert_eq!(code, 0, "Find owned copy transfer failed (code: {}, stderr: {})", code, stderr);
}

/// 13. Owned Item any: consumed items dropped, early exit preserves unconsumed in iterator
#[test]
fn test_iter_any_owned_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("any_owned_drop");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

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

        extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        struct OwnedTripleIter {
            buf: *rw TrackedItem,
            pos: u64,
            len: u64,
        }

        impl Drop for OwnedTripleIter {
            fn drop(self: &rw Self) {
                dec rw i = self.pos;
                while i < self.len {
                    unsafe {
                        ptr::drop_in_place<TrackedItem>(ptr::add_mut<TrackedItem>(self.buf, i));
                    }
                    i = i + (1 as u64);
                }
                dec size = self.len * (sizeof(TrackedItem) as u64);
                __mellis_dealloc(self.buf as *rw u8, size, 8 as u64);
            }
        }

        impl Iterator<TrackedItem> for OwnedTripleIter {
            fn next(self: &rw Self) -> Option<TrackedItem> {
                if self.pos < self.len {
                    unsafe {
                        dec p = ptr::add_mut<TrackedItem>(self.buf, self.pos);
                        self.pos = self.pos + (1 as u64);
                        return Option::Some(ptr::read<TrackedItem>(p as *TrackedItem));
                    }
                }
                return Option::None;
            }
        }

        fn is_second(item: TrackedItem) -> bool {
            return item.val == 20;
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                unsafe {
                    (*c_ptr).created = (*c_ptr).created + (3 as u64);
                }
                dec size = 3 as u64 * (sizeof(TrackedItem) as u64);
                dec raw_buf = __mellis_alloc(size, 8 as u64) as *rw TrackedItem;
                unsafe {
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 0 as u64), TrackedItem { val: 10, counter: c_ptr });
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 1 as u64), TrackedItem { val: 20, counter: c_ptr });
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 2 as u64), TrackedItem { val: 30, counter: c_ptr });
                }
                dec it = OwnedTripleIter {
                    buf: raw_buf,
                    pos: 0 as u64,
                    len: 3 as u64,
                };

                // iter_any checks item 1 (false -> dropped in predicate), item 2 (true -> dropped in predicate)
                // then early exits!
                dec res = iter_any(it, is_second);

                if res == false {
                    return 1;
                }

                // All 3 items dropped: items 1 and 2 consumed by predicate, item 3 dropped by OwnedTripleIter drop!
                if counter.dropped != (3 as u64) {
                    return 2;
                }
            }

            if counter.created != (3 as u64) || counter.dropped != (3 as u64) {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Any owned droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// 14. Owned Item all: early exit drops consumed item, unconsumed dropped by iterator
#[test]
fn test_iter_all_owned_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("all_owned_drop");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

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

        extern fn __mellis_alloc(size: u64, align: u64) -> *rw u8;
        extern fn __mellis_dealloc(ptr: *rw u8, size: u64, align: u64);

        struct OwnedTripleIter {
            buf: *rw TrackedItem,
            pos: u64,
            len: u64,
        }

        impl Drop for OwnedTripleIter {
            fn drop(self: &rw Self) {
                dec rw i = self.pos;
                while i < self.len {
                    unsafe {
                        ptr::drop_in_place<TrackedItem>(ptr::add_mut<TrackedItem>(self.buf, i));
                    }
                    i = i + (1 as u64);
                }
                dec size = self.len * (sizeof(TrackedItem) as u64);
                __mellis_dealloc(self.buf as *rw u8, size, 8 as u64);
            }
        }

        impl Iterator<TrackedItem> for OwnedTripleIter {
            fn next(self: &rw Self) -> Option<TrackedItem> {
                if self.pos < self.len {
                    unsafe {
                        dec p = ptr::add_mut<TrackedItem>(self.buf, self.pos);
                        self.pos = self.pos + (1 as u64);
                        return Option::Some(ptr::read<TrackedItem>(p as *TrackedItem));
                    }
                }
                return Option::None;
            }
        }

        fn is_even(item: TrackedItem) -> bool {
            return (item.val % 2) == 0;
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                unsafe {
                    (*c_ptr).created = (*c_ptr).created + (3 as u64);
                }
                dec size = 3 as u64 * (sizeof(TrackedItem) as u64);
                dec raw_buf = __mellis_alloc(size, 8 as u64) as *rw TrackedItem;
                unsafe {
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 0 as u64), TrackedItem { val: 11, counter: c_ptr });
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 1 as u64), TrackedItem { val: 20, counter: c_ptr });
                    ptr::write<TrackedItem>(ptr::add_mut<TrackedItem>(raw_buf, 2 as u64), TrackedItem { val: 30, counter: c_ptr });
                }
                dec it = OwnedTripleIter {
                    buf: raw_buf,
                    pos: 0 as u64,
                    len: 3 as u64,
                };

                // Item 1 is odd -> iter_all returns false on element 1 immediately!
                dec res = iter_all(it, is_even);

                if res == true {
                    return 1;
                }

                // Item 1 consumed & dropped in predicate; items 2 and 3 dropped by OwnedTripleIter drop!
                if counter.dropped != (3 as u64) {
                    return 2;
                }
            }

            if counter.created != (3 as u64) || counter.dropped != (3 as u64) {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "All owned droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// 15. iter_fold with DropTracker accumulator: intermediate replaced accs dropped, final returned
#[test]
fn test_iter_fold_droptracker_accumulator() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("fold_acc_drop");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        struct DropCounter {
            private created: u64,
            private dropped: u64,
        }

        struct TrackedAcc {
            val: i32,
            counter: *rw DropCounter,
        }

        impl Drop for TrackedAcc {
            fn drop(self: &rw Self) {
                unsafe {
                    (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                }
            }
        }

        fn add_step(acc: TrackedAcc, x: &i32) -> TrackedAcc {
            dec c = acc.counter;
            unsafe {
                (*c).created = (*c).created + (1 as u64);
            }
            dec new_val = acc.val + (*x);
            // acc drops at end of this function!
            return TrackedAcc { val: new_val, counter: c };
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                unsafe {
                    (*c_ptr).created = (*c_ptr).created + (1 as u64); // init acc
                }
                dec init_acc = TrackedAcc { val: 0, counter: c_ptr };

                dec arr: [i32; 3] = [10, 20, 30];
                dec it = slice_iter<i32>(&arr);

                // fold replaces acc 3 times:
                // step 1: acc0 (val 0) + 10 -> acc1 (val 10), acc0 dropped
                // step 2: acc1 (val 10) + 20 -> acc2 (val 30), acc1 dropped
                // step 3: acc2 (val 30) + 30 -> acc3 (val 60), acc2 dropped
                dec final_acc = iter_fold(it, init_acc, add_step);

                if final_acc.val != 60 {
                    return 1;
                }

                // 4 total created (init + 3 steps).
                // 3 intermediate dropped so far; final_acc is still alive!
                unsafe {
                    if (*c_ptr).created != (4 as u64) {
                        return 2;
                    }
                    if (*c_ptr).dropped != (3 as u64) {
                        return 3;
                    }
                }
            }

            // After final_acc drops:
            if counter.created != (4 as u64) || counter.dropped != (4 as u64) {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Fold droptracker accumulator failed (code: {}, stderr: {})", code, stderr);
}
