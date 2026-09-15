use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_collect_{}_{}_{}",
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
        for d in &err {
            println!("DIAG: {:?} at span {:?}", d.message, d.span);
        }
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

/// C1: Basic iter_collect_vec from SliceIter with deref map
#[test]
fn test_c1_collect_vec_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c1_vec_basic");

    let src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec arr: [i32; 5] = [10, 20, 30, 40, 50];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, deref_i32);
            dec v = iter_collect_vec(m_it);

            if v.len() != (5 as u64) {
                return 1;
            }
            match v.get(0 as u64) {
                Option::Some(r) -> {
                    if *r != 10 {
                        return 2;
                    }
                },
                Option::None -> {
                    return 20;
                },
            }
            match v.get(2 as u64) {
                Option::Some(r) -> {
                    if *r != 30 {
                        return 3;
                    }
                },
                Option::None -> {
                    return 30;
                },
            }
            match v.get(4 as u64) {
                Option::Some(r) -> {
                    if *r != 50 {
                        return 4;
                    }
                },
                Option::None -> {
                    return 40;
                },
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C1 collect_vec basic failed (code: {}, stderr: {})", code, stderr);
}

/// C2: Empty iter_collect_vec
#[test]
fn test_c2_collect_vec_empty() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c2_vec_empty");

    let src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec arr: [i32; 0] = [];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, deref_i32);
            dec v = iter_collect_vec(m_it);

            if v.len() != (0 as u64) {
                return 1;
            }
            if v.is_empty() == false {
                return 2;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C2 collect_vec empty failed (code: {}, stderr: {})", code, stderr);
}

/// C3: iter_collect_vec preserves value ownership (non-trivial element transfer)
#[test]
fn test_c3_collect_vec_ownership_transfer() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c3_vec_ownership");

    let src = r#"
        import <core>;
        import <alloc>;

        struct Point {
            x: i32,
            y: i32,
        }

        struct PointGenerator {
            count: i32,
            max: i32,
        }

        impl Iterator<Point> for PointGenerator {
            fn next(self: &rw Self) -> Option<Point> {
                if self.count < self.max {
                    dec pt = Point { x: self.count * 10, y: self.count * 100 };
                    self.count = self.count + 1;
                    return Option::Some(pt);
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec gen = PointGenerator { count: 0, max: 3 };
            dec v = iter_collect_vec(gen);

            if v.len() != (3 as u64) {
                return 1;
            }
            match v.get(0 as u64) {
                Option::Some(p) -> {
                    if (*p).x != 0 || (*p).y != 0 {
                        return 2;
                    }
                },
                Option::None -> {
                    return 20;
                },
            }
            match v.get(1 as u64) {
                Option::Some(p) -> {
                    if (*p).x != 10 || (*p).y != 100 {
                        return 3;
                    }
                },
                Option::None -> {
                    return 30;
                },
            }
            match v.get(2 as u64) {
                Option::Some(p) -> {
                    if (*p).x != 20 || (*p).y != 200 {
                        return 4;
                    }
                },
                Option::None -> {
                    return 40;
                },
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C3 collect_vec ownership transfer failed (code: {}, stderr: {})", code, stderr);
}

/// C4: iter_collect_vec with DropTracker (owned element transfer, zero early drops, exact drops on exit)
#[test]
fn test_c4_collect_vec_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c4_vec_droptracker");

    let src = r#"
        import <core>;
        import <alloc>;

        struct DropCounter {
            created: u64,
            dropped: u64,
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

        struct TrackedGenerator {
            cur: i32,
            max: i32,
            counter: *rw DropCounter,
        }

        impl Iterator<TrackedItem> for TrackedGenerator {
            fn next(self: &rw Self) -> Option<TrackedItem> {
                if self.cur < self.max {
                    unsafe {
                        (*self.counter).created = (*self.counter).created + (1 as u64);
                    }
                    dec item = TrackedItem { val: self.cur, counter: self.counter };
                    self.cur = self.cur + 1;
                    return Option::Some(item);
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                dec gen = TrackedGenerator { cur: 1, max: 6, counter: c_ptr };
                dec v = iter_collect_vec(gen);

                if v.len() != (5 as u64) {
                    return 1;
                }

                // During lifetime of Vec: 5 created, 0 dropped!
                if counter.created != (5 as u64) {
                    return 2;
                }
                if counter.dropped != (0 as u64) {
                    return 3;
                }
            }

            // After Vec dropped: exactly 5 items dropped!
            if counter.dropped != (5 as u64) {
                return 4;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C4 collect_vec droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// C5: Basic iter_collect_hashset from distinct elements
#[test]
fn test_c5_collect_hashset_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c5_set_basic");

    let src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec arr: [i32; 3] = [10, 20, 30];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, deref_i32);
            dec s = iter_collect_hashset(m_it);

            if s.len() != (3 as u64) {
                return 1;
            }
            dec k10 = 10;
            dec k20 = 20;
            dec k30 = 30;
            dec k99 = 99;
            if s.contains(&k10) == false {
                return 2;
            }
            if s.contains(&k20) == false {
                return 3;
            }
            if s.contains(&k30) == false {
                return 4;
            }
            if s.contains(&k99) == true {
                return 5;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C5 collect_hashset basic failed (code: {}, stderr: {})", code, stderr);
}

/// C6: iter_collect_hashset duplicate deduplication
#[test]
fn test_c6_collect_hashset_dedup() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c6_set_dedup");

    let src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec arr: [i32; 7] = [1, 2, 2, 3, 1, 4, 3];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, deref_i32);
            dec s = iter_collect_hashset(m_it);

            // 7 items in stream, 4 unique: 1, 2, 3, 4
            if s.len() != (4 as u64) {
                return 1;
            }
            dec k1 = 1;
            dec k2 = 2;
            dec k3 = 3;
            dec k4 = 4;
            dec k5 = 5;
            if s.contains(&k1) == false || s.contains(&k2) == false || s.contains(&k3) == false || s.contains(&k4) == false {
                return 2;
            }
            if s.contains(&k5) == true {
                return 3;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C6 collect_hashset dedup failed (code: {}, stderr: {})", code, stderr);
}

/// C7: iter_collect_hashset with DropTracker (duplicates dropped during insertion, unique dropped on exit)
#[test]
fn test_c7_collect_hashset_droptracker() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c7_set_droptracker");

    let src = r#"
        import <core>;
        import <alloc>;

        struct DropCounter {
            created: u64,
            dropped: u64,
        }

        struct TrackedItem {
            val: i32,
            counter: *rw DropCounter,
        }

        impl Hash for TrackedItem {
            fn hash(self: &Self) -> u64 {
                return self.val.hash();
            }
        }

        impl Eq for TrackedItem {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.val == other.val;
            }
        }

        impl Drop for TrackedItem {
            fn drop(self: &rw Self) {
                unsafe {
                    (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                }
            }
        }

        struct DedupGenerator {
            step: i32,
            counter: *rw DropCounter,
        }

        impl Iterator<TrackedItem> for DedupGenerator {
            fn next(self: &rw Self) -> Option<TrackedItem> {
                // Sequence: 10, 20, 10 (dup), 30, 20 (dup) -> 5 created, 3 unique
                if self.step == 0 {
                    unsafe { (*self.counter).created = (*self.counter).created + (1 as u64); }
                    self.step = 1;
                    return Option::Some(TrackedItem { val: 10, counter: self.counter });
                } else if self.step == 1 {
                    unsafe { (*self.counter).created = (*self.counter).created + (1 as u64); }
                    self.step = 2;
                    return Option::Some(TrackedItem { val: 20, counter: self.counter });
                } else if self.step == 2 {
                    unsafe { (*self.counter).created = (*self.counter).created + (1 as u64); }
                    self.step = 3;
                    return Option::Some(TrackedItem { val: 10, counter: self.counter });
                } else if self.step == 3 {
                    unsafe { (*self.counter).created = (*self.counter).created + (1 as u64); }
                    self.step = 4;
                    return Option::Some(TrackedItem { val: 30, counter: self.counter });
                } else if self.step == 4 {
                    unsafe { (*self.counter).created = (*self.counter).created + (1 as u64); }
                    self.step = 5;
                    return Option::Some(TrackedItem { val: 20, counter: self.counter });
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec rw counter = DropCounter { created: 0 as u64, dropped: 0 as u64 };
            dec c_ptr = &rw counter as *rw DropCounter;

            {
                dec gen = DedupGenerator { step: 0, counter: c_ptr };
                dec s = iter_collect_hashset(gen);

                if s.len() != (3 as u64) {
                    return 1;
                }

                // 5 items created in total
                if counter.created != (5 as u64) {
                    return 2;
                }

                // 2 duplicate items dropped during collection!
                if counter.dropped != (2 as u64) {
                    return 3;
                }
            }

            // After HashSet drops: all 5 items dropped!
            if counter.dropped != (5 as u64) {
                return 4;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C7 collect_hashset droptracker failed (code: {}, stderr: {})", code, stderr);
}

/// C8: Basic iter_collect_hashmap from pair stream
#[test]
fn test_c8_collect_hashmap_basic() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c8_map_basic");

    let src = r#"
        import <core>;
        import <alloc>;

        struct PairGenerator {
            cur: i32,
            max: i32,
        }

        impl Iterator<(i32, i32)> for PairGenerator {
            fn next(self: &rw Self) -> Option<(i32, i32)> {
                if self.cur < self.max {
                    dec k = self.cur;
                    dec v = self.cur * 10;
                    self.cur = self.cur + 1;
                    return Option::Some((k, v));
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec gen = PairGenerator { cur: 1, max: 4 };
            dec m = iter_collect_hashmap(gen);

            if m.len() != (3 as u64) {
                return 1;
            }

            dec k1 = 1;
            dec v1 = match m.get(&k1) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v1 != 10 {
                return 2;
            }

            dec k2 = 2;
            dec v2 = match m.get(&k2) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v2 != 20 {
                return 3;
            }

            dec k3 = 3;
            dec v3 = match m.get(&k3) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v3 != 30 {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C8 collect_hashmap basic failed (code: {}, stderr: {})", code, stderr);
}

/// C9: iter_collect_hashmap duplicate key last-wins value replacement
#[test]
fn test_c9_collect_hashmap_dedup_last_wins() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c9_map_last_wins");

    let src = r#"
        import <core>;
        import <alloc>;

        struct DupPairGenerator {
            step: i32,
        }

        impl Iterator<(i32, i32)> for DupPairGenerator {
            fn next(self: &rw Self) -> Option<(i32, i32)> {
                // (1, 100), (2, 200), (1, 999) -> key 1 overwritten with 999
                if self.step == 0 {
                    self.step = 1;
                    return Option::Some((1, 100));
                } else if self.step == 1 {
                    self.step = 2;
                    return Option::Some((2, 200));
                } else if self.step == 2 {
                    self.step = 3;
                    return Option::Some((1, 999));
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec gen = DupPairGenerator { step: 0 };
            dec m = iter_collect_hashmap(gen);

            if m.len() != (2 as u64) {
                return 1;
            }

            dec k1 = 1;
            dec v1 = match m.get(&k1) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v1 != 999 {
                return 2;
            }

            dec k2 = 2;
            dec v2 = match m.get(&k2) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v2 != 200 {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C9 collect_hashmap last-wins failed (code: {}, stderr: {})", code, stderr);
}

/// C10: iter_collect_hashmap with DropTracker key + value (identity verification!)
#[test]
fn test_c10_collect_hashmap_droptracker_identity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c10_map_droptracker_identity");

    let src = r#"
        import <core>;
        import <alloc>;

        struct Counter {
            created: u64,
            dropped: u64,
        }

        struct TrackedKey {
            id: i32,
            marker: i32,
            counter: *rw Counter,
        }

        impl Hash for TrackedKey {
            fn hash(self: &Self) -> u64 {
                return self.id.hash();
            }
        }

        impl Eq for TrackedKey {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        impl Drop for TrackedKey {
            fn drop(self: &rw Self) {
                if (self.counter as u64) != (0 as u64) {
                    unsafe {
                        (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                    }
                }
            }
        }

        struct TrackedVal {
            id: i32,
            marker: i32,
            counter: *rw Counter,
        }

        impl Drop for TrackedVal {
            fn drop(self: &rw Self) {
                if (self.counter as u64) != (0 as u64) {
                    unsafe {
                        (*self.counter).dropped = (*self.counter).dropped + (1 as u64);
                    }
                }
            }
        }

        struct PairGenerator {
            step: i32,
            k_counter: *rw Counter,
            v_counter: *rw Counter,
        }

        impl Iterator<(TrackedKey, TrackedVal)> for PairGenerator {
            fn next(self: &rw Self) -> Option<(TrackedKey, TrackedVal)> {
                // Pair 1: key(id=1, marker=10), val(id=1, marker=100)
                // Pair 2: key(id=1, marker=20), val(id=1, marker=200) (duplicate key!)
                if self.step == 0 {
                    unsafe {
                        (*self.k_counter).created = (*self.k_counter).created + (1 as u64);
                        (*self.v_counter).created = (*self.v_counter).created + (1 as u64);
                    }
                    self.step = 1;
                    return Option::Some((
                        TrackedKey { id: 1, marker: 10, counter: self.k_counter },
                        TrackedVal { id: 1, marker: 100, counter: self.v_counter },
                    ));
                } else if self.step == 1 {
                    unsafe {
                        (*self.k_counter).created = (*self.k_counter).created + (1 as u64);
                        (*self.v_counter).created = (*self.v_counter).created + (1 as u64);
                    }
                    self.step = 2;
                    return Option::Some((
                        TrackedKey { id: 1, marker: 20, counter: self.k_counter },
                        TrackedVal { id: 1, marker: 200, counter: self.v_counter },
                    ));
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec rw k_counter = Counter { created: 0 as u64, dropped: 0 as u64 };
            dec rw v_counter = Counter { created: 0 as u64, dropped: 0 as u64 };
            dec k_ptr = &rw k_counter as *rw Counter;
            dec v_ptr = &rw v_counter as *rw Counter;

            {
                dec gen = PairGenerator { step: 0, k_counter: k_ptr, v_counter: v_ptr };
                dec m = iter_collect_hashmap(gen);

                if m.len() != (1 as u64) {
                    return 1;
                }

                // Verify identity:
                // 1. Value must have marker == 200 (last-wins value replacement)
                // Search key uses null counter so it doesn't affect tracking
                dec search_key = TrackedKey { id: 1, marker: 0, counter: 0 as u64 as *rw Counter };
                dec val_marker = match m.get(&search_key) {
                    Option::Some(v) -> (*v).marker,
                    Option::None -> 0,
                };
                if val_marker != 200 {
                    return 2; // Value was not replaced!
                }

                // 2. Key must have marker == 10 (first-inserted key preserved per HM-4 / HS-3)
                dec rw surviving_marker = 0;
                {
                    dec rw k_iter = m.keys();
                    match k_iter.next() {
                        Option::Some(k) -> {
                            surviving_marker = (*k).marker;
                        },
                        Option::None -> {},
                    }
                }
                if surviving_marker != 10 {
                    return 3; // First key was not preserved!
                }

                // Drops during collect: 1 incoming dup key dropped, 1 old replaced value dropped
                if k_counter.dropped != (1 as u64) {
                    return 4;
                }
                if v_counter.dropped != (1 as u64) {
                    return 5;
                }
            }

            // After HashMap drops: all created keys and values must be dropped exactly once
            if k_counter.created != (2 as u64) || k_counter.dropped != (2 as u64) {
                return 6;
            }
            if v_counter.created != (2 as u64) || v_counter.dropped != (2 as u64) {
                return 7;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C10 collect_hashmap droptracker identity failed (code: {}, stderr: {})", code, stderr);
}

/// C11: Multi-adapter pipeline chain into collect_vec
#[test]
fn test_c11_collect_pipeline_chain() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c11_pipeline_chain");

    let src = r#"
        import <core>;
        import <alloc>;

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn square(x: &i32) -> i32 {
            return (*x) * (*x);
        }

        fn main() -> i32 {
            dec arr: [i32; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            dec it = slice_iter<i32>(&arr);
            dec f_it = iter_filter(it, is_even);
            dec m_it = iter_map(f_it, square);
            dec t_it = iter_take(m_it, 3 as u64);
            dec v = iter_collect_vec(t_it);

            // Evens: 2, 4, 6, 8, 10
            // Squares: 4, 16, 36, 64, 100
            // Take 3: 4, 16, 36
            if v.len() != (3 as u64) {
                return 1;
            }

            match v.get(0 as u64) {
                Option::Some(r) -> { if *r != 4 { return 2; } },
                Option::None -> { return 20; },
            }
            match v.get(1 as u64) {
                Option::Some(r) -> { if *r != 16 { return 3; } },
                Option::None -> { return 30; },
            }
            match v.get(2 as u64) {
                Option::Some(r) -> { if *r != 36 { return 4; } },
                Option::None -> { return 40; },
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C11 pipeline chain failed (code: {}, stderr: {})", code, stderr);
}

/// C12: Source vs .llib Parity: run iter_collect_vec and iter_collect_hashmap against precompiled alloc.llib
#[test]
fn test_c12_collect_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c12_parity");

    let alloc_llib = sysroot.root().join("libs").join("external").join("alloc.llib");
    assert!(alloc_llib.exists(), "alloc.llib must exist for parity test");

    let src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        struct PairGenerator {
            cur: i32,
            max: i32,
        }

        impl Iterator<(i32, i32)> for PairGenerator {
            fn next(self: &rw Self) -> Option<(i32, i32)> {
                if self.cur < self.max {
                    dec k = self.cur;
                    dec v = self.cur * 100;
                    self.cur = self.cur + 1;
                    return Option::Some((k, v));
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec arr: [i32; 4] = [100, 200, 300, 400];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, deref_i32);
            dec v = iter_collect_vec(m_it);

            if v.len() != (4 as u64) {
                return 1;
            }

            dec gen = PairGenerator { cur: 1, max: 3 };
            dec m = iter_collect_hashmap(gen);

            if m.len() != (2 as u64) {
                return 2;
            }

            dec k1 = 1;
            match m.get(&k1) {
                Option::Some(val) -> {
                    if *val != 100 {
                        return 3;
                    }
                },
                Option::None -> {
                    return 30;
                },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C12 parity failed (code: {}, stderr: {})", code, stderr);
}

/// C13: Borrowed-to-owned collection via map (&i32 -> Box<i32> -> Vec<Box<i32>>)
#[test]
fn test_c13_collect_borrowed_to_owned_via_map() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c13_borrow_to_owned");

    let src = r#"
        import <core>;
        import <alloc>;

        fn to_boxed_i32(x: &i32) -> Box<i32> {
            return std::box_new<i32>(*x);
        }

        fn main() -> i32 {
            dec arr: [i32; 3] = [111, 222, 333];
            dec it = slice_iter<i32>(&arr);
            dec m_it = iter_map(it, to_boxed_i32);
            dec v = iter_collect_vec(m_it);

            if v.len() != (3 as u64) {
                return 1;
            }

            match v.get(0 as u64) {
                Option::Some(b) -> {
                    if *((*b).as_ref()) != 111 {
                        return 2;
                    }
                },
                Option::None -> {
                    return 20;
                },
            }
            match v.get(1 as u64) {
                Option::Some(b) -> {
                    if *((*b).as_ref()) != 222 {
                        return 3;
                    }
                },
                Option::None -> {
                    return 30;
                },
            }
            match v.get(2 as u64) {
                Option::Some(b) -> {
                    if *((*b).as_ref()) != 333 {
                        return 4;
                    }
                },
                Option::None -> {
                    return 40;
                },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C13 borrowed to owned via map failed (code: {}, stderr: {})", code, stderr);
}

/// C14: Borrowed collection lifetime lock
/// Part A: Borrowck rejects mutating source while a borrow is live.
/// Part B: Mutating source after borrow dies succeeds.
#[test]
fn test_c14_collect_borrowed_lifetime_lock() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c14_borrow_lock");

    // Part A: Borrowck must reject mutating source collection while live borrow exists
    let reject_src = r#"
        import <core>;
        import <alloc>;

        fn main() -> i32 {
            dec rw src = vec_with_capacity<i32>(4 as u64);
            src.push(10);
            src.push(20);

            dec r = src.get(0 as u64);

            // MUTATION WHILE BORROW IS LIVE: Must be rejected by Borrowck!
            src.push(30);

            match r {
                Option::Some(x) -> { dec v = *x; },
                Option::None -> {},
            }
            return 0;
        }
    "#;

    let reject_path = dir.join("reject_lock.ln");
    fs::write(&reject_path, reject_src).unwrap();

    let mut check_opts = CompilerOptions::default();
    check_opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let check_res = check(reject_path.to_str().unwrap(), reject_src.to_string(), &check_opts);
    assert!(
        check_res.is_err(),
        "Borrowck MUST reject mutation while borrow is live"
    );
    let diags = check_res.unwrap_err();
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
        "Expected borrow error for mutating Vec during live borrow, got: {:?}",
        diags
    );

    // Part B: Mutation after borrow has ended succeeds and runs cleanly!
    let run_src = r#"
        import <core>;
        import <alloc>;

        fn deref_i32(x: &i32) -> i32 {
            return *x;
        }

        fn main() -> i32 {
            dec rw src = vec_with_capacity<i32>(4 as u64);
            src.push(10);
            src.push(20);

            // Scope containing borrow ends before mutation
            {
                dec it = slice_iter<i32>(src.as_slice());
                dec m_it = iter_map(it, deref_i32);
                dec collected = iter_collect_vec(m_it);
                if collected.len() != (2 as u64) {
                    return 1;
                }
            }

            // Mutation after borrow ends: completely valid!
            src.push(30);

            if src.len() != (3 as u64) {
                return 2;
            }
            return 0;
        }
    "#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, run_src, &check_opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C14 mutation after borrow death failed (code: {}, stderr: {})", code, stderr);
}

/// C15: HashSet duplicate representative semantics (verify HS-3: existing representative preserved!)
#[test]
fn test_c15_collect_hashset_duplicate_representative_semantics() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("c15_set_representative");

    let src = r#"
        import <core>;
        import <alloc>;

        struct ItemWithIdentity {
            id: i32,
            marker: i32,
        }

        impl Hash for ItemWithIdentity {
            fn hash(self: &Self) -> u64 {
                return self.id.hash();
            }
        }

        impl Eq for ItemWithIdentity {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        struct IdentityGenerator {
            step: i32,
        }

        impl Iterator<ItemWithIdentity> for IdentityGenerator {
            fn next(self: &rw Self) -> Option<ItemWithIdentity> {
                // Yield two items with identical id=42, but distinct markers (100 vs 200)
                if self.step == 0 {
                    self.step = 1;
                    return Option::Some(ItemWithIdentity { id: 42, marker: 100 });
                } else if self.step == 1 {
                    self.step = 2;
                    return Option::Some(ItemWithIdentity { id: 42, marker: 200 });
                }
                return Option::None;
            }
        }

        fn main() -> i32 {
            dec gen = IdentityGenerator { step: 0 };
            dec s = iter_collect_hashset(gen);

            if s.len() != (1 as u64) {
                return 1;
            }

            // Iterate the set to observe WHICH representative survived:
            // HS-3 requires: existing key is KEPT, incoming duplicate is discarded.
            // So marker MUST be 100!
            dec rw it = s.iter();
            match it.next() {
                Option::Some(item) -> {
                    if (*item).marker != 100 {
                        return 2; // Incoming duplicate replaced existing item, violates HS-3!
                    }
                },
                Option::None -> {
                    return 20;
                },
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "C15 duplicate representative semantics failed (code: {}, stderr: {})", code, stderr);
}
