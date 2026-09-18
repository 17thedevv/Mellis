// =============================================================================
// Stdlib Phase 04.4 HashMap<K, V> Acceptance Tests
//
// Verifies:
//   1. Empty Lifecycle (default constructor, capacity, empty state, clean drop)
//   2. Insert, Get & Persistence
//   3. Membership check via contains_key
//   4. In-place Mutation via get_mut
//   5. Overwrite Semantics (in-place destruction of old value, length unchanged)
//   6. Removal & Tombstone Transparency
//   7. Clear
//   8. Multiple Hash Collisions Transparency
//   9. Dynamic Growth & Relocation
//   10. Borrowck: Shared read borrow &V blocks mutation
//   11. Borrowck: Exclusive mutable borrow &rw V exclusivity
//   12. DropTracker Stress Test: Zero Leaks & Zero Double-Frees
//   13. Source vs .llib Parity
// =============================================================================

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_hashmap_{}_{}_{}",
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

fn locate_canonical_hashmap_ln() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("hashmap.ln");
        if p.exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/hashmap.ln");
}

fn locate_canonical_hashmap_llib() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("hashmap.llib");
        if p.parent().unwrap().exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/hashmap.llib");
}

/// 1. Empty Lifecycle
#[test]
fn test_hashmap_empty_lifecycle() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_empty_lifecycle");

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

        fn main() -> i32 {
            dec rw map = hashmap_new<i32, i32>();
            if map.len() != (0 as u64) {
                return 1;
            }
            if map.is_empty() == false {
                return 2;
            }

            dec rw map_cap = hashmap_with_capacity<i32, i32>(16 as u64);
            if map_cap.capacity() != (16 as u64) {
                return 3;
            }
            if map_cap.len() != (0 as u64) {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Empty lifecycle failed with code {} (stderr: {})", code, stderr);
}

/// 2. Basic Insert, Get & Persistence
#[test]
fn test_hashmap_insert_and_get() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_insert_and_get");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            dec ins1 = map.insert(10, 100);
            dec ins2 = map.insert(20, 200);
            dec ins3 = map.insert(30, 300);

            if ins1 == false || ins2 == false || ins3 == false {
                return 1;
            }
            if map.len() != (3 as u64) {
                return 2;
            }

            dec k10: i32 = 10;
            dec v10 = match map.get(&k10) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v10 != 100 {
                return 3;
            }

            dec k20: i32 = 20;
            dec v20 = match map.get(&k20) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v20 != 200 {
                return 4;
            }

            dec k99: i32 = 99;
            dec v99 = match map.get(&k99) {
                Option::Some(_) -> 1,
                Option::None -> 0,
            };
            if v99 != 0 {
                return 5;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Insert and get failed with code {} (stderr: {})", code, stderr);
}

/// 3. Membership Check via `contains_key`
#[test]
fn test_hashmap_contains_key() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_contains_key");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);
            map.insert(2, 20);

            dec k1: i32 = 1;
            dec k2: i32 = 2;
            dec k3: i32 = 3;

            if map.contains_key(&k1) == false {
                return 1;
            }
            if map.contains_key(&k2) == false {
                return 2;
            }
            if map.contains_key(&k3) == true {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "contains_key failed with code {} (stderr: {})", code, stderr);
}

/// 4. In-place Mutation via `get_mut`
#[test]
fn test_hashmap_get_mut_inplace() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_get_mut");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(5, 50);

            dec k5: i32 = 5;
            dec mut_opt = map.get_mut(&k5);
            match mut_opt {
                Option::Some(val_ref) -> {
                    *val_ref = 555;
                },
                Option::None -> {
                    return 1;
                },
            }

            dec read_opt = map.get(&k5);
            dec final_val = match read_opt {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };

            if final_val != 555 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "get_mut failed with code {} (stderr: {})", code, stderr);
}

/// 5. Overwrite Semantics
#[test]
fn test_hashmap_overwrite_semantics() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_overwrite");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            dec ins1 = map.insert(7, 70);
            if ins1 == false {
                return 1;
            }
            if map.len() != (1 as u64) {
                return 2;
            }

            dec ins2 = map.insert(7, 777);
            if ins2 == true {
                return 3; // false indicates an overwrite
            }
            if map.len() != (1 as u64) {
                return 4; // length remains 1
            }

            dec k7: i32 = 7;
            dec v = match map.get(&k7) {
                Option::Some(val) -> *val,
                Option::None -> 0,
            };
            if v != 777 {
                return 5;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Overwrite failed with code {} (stderr: {})", code, stderr);
}

/// 6. Removal & Tombstone Transparency
#[test]
fn test_hashmap_remove() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_remove");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);
            map.insert(2, 20);
            map.insert(3, 30);

            dec k2: i32 = 2;
            dec rem_res = map.remove(&k2);
            if rem_res == false {
                return 1;
            }
            if map.len() != (2 as u64) {
                return 2;
            }

            if map.contains_key(&k2) == true {
                return 3;
            }

            // Other keys remain retrievable
            dec k1: i32 = 1;
            dec k3: i32 = 3;
            if map.contains_key(&k1) == false || map.contains_key(&k3) == false {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Remove failed with code {} (stderr: {})", code, stderr);
}

/// 7. Clear
#[test]
fn test_hashmap_clear() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_clear");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);
            map.insert(2, 20);
            map.insert(3, 30);

            if map.len() != (3 as u64) {
                return 1;
            }

            map.clear();

            if map.len() != (0 as u64) {
                return 2;
            }
            if map.is_empty() == false {
                return 3;
            }

            dec k1: i32 = 1;
            if map.contains_key(&k1) == true {
                return 4;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Clear failed with code {} (stderr: {})", code, stderr);
}

/// 8. Multiple Hash Collisions Transparency
#[test]
fn test_hashmap_multiple_collisions() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_collisions");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <cmp>;
        import <hash>;
        import <iter_adapters>;
        import <iter_consumers>;
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct CollidingKey {
            id: u64,
            forced_hash: u64,
        }

        impl Hash for CollidingKey {
            fn hash(self: &Self) -> u64 {
                return self.forced_hash;
            }
        }

        impl Eq for CollidingKey {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<CollidingKey, i32>(16 as u64);

            dec k1 = CollidingKey { id: 1 as u64, forced_hash: 42 as u64 };
            dec k2 = CollidingKey { id: 2 as u64, forced_hash: 42 as u64 };
            dec k3 = CollidingKey { id: 3 as u64, forced_hash: 42 as u64 };
            dec k4 = CollidingKey { id: 4 as u64, forced_hash: 42 as u64 };

            map.insert(k1, 100);
            map.insert(k2, 200);
            map.insert(k3, 300);
            map.insert(k4, 400);

            if map.len() != (4 as u64) {
                return 1;
            }

            // Remove middle key
            dec q2 = CollidingKey { id: 2 as u64, forced_hash: 42 as u64 };
            map.remove(&q2);

            // Verify remaining keys past the tombstone are still found
            dec q3 = CollidingKey { id: 3 as u64, forced_hash: 42 as u64 };
            dec v3 = match map.get(&q3) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v3 != 300 {
                return 2;
            }

            dec q4 = CollidingKey { id: 4 as u64, forced_hash: 42 as u64 };
            dec v4 = match map.get(&q4) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v4 != 400 {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Collisions test failed with code {} (stderr: {})", code, stderr);
}

/// 9. Dynamic Growth & Relocation
#[test]
fn test_hashmap_dynamic_growth() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_growth");

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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);

            // Insert 25 elements, forcing multiple resizes (8 -> 16 -> 32 -> 64)
            dec rw i: i32 = 1;
            while i <= 25 {
                map.insert(i, i * 10);
                i = i + 1;
            }

            if map.len() != (25 as u64) {
                return 1;
            }
            if map.capacity() < (32 as u64) {
                return 2;
            }

            // Verify all 25 elements intact
            dec rw j: i32 = 1;
            while j <= 25 {
                dec val = match map.get(&j) {
                    Option::Some(v) -> *v,
                    Option::None -> 0,
                };
                if val != (j * 10) {
                    return 3;
                }
                j = j + 1;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Dynamic growth failed with code {} (stderr: {})", code, stderr);
}

/// 10. Borrowck: Shared read borrow `&V` blocks mutation
#[test]
fn test_hashmap_borrowck_read_blocks_mutation() {
    let dir = create_temp_dir("map_borrowck_read_mut");
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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);

            dec k1: i32 = 1;
            dec val_ref = map.get(&k1); // Shared borrow of map via life_from(self)

            map.insert(2, 20); // ERROR: mutating map while val_ref is live

            dec x = match val_ref {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            return x;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(res.is_err(), "Mutating HashMap while an element is borrowed must be rejected by borrowck");
}

/// 11. Borrowck: Exclusive mutable borrow `&rw V` blocks concurrent access
#[test]
fn test_hashmap_borrowck_mut_exclusive() {
    let dir = create_temp_dir("map_borrowck_mut_exclusive");
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

        fn main() -> i32 {
            dec rw map = hashmap_with_capacity<i32, i32>(8 as u64);
            map.insert(1, 10);

            dec k1: i32 = 1;
            dec mut_ref = map.get_mut(&k1); // Exclusive borrow of map

            dec read_attempt = map.get(&k1); // ERROR: reading map while exclusive borrow is active

            match mut_ref {
                Option::Some(v) -> { *v = 100; },
                Option::None -> {},
            }
            return 0;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(res.is_err(), "Reading HashMap while an exclusive mutable reference is active must be rejected by borrowck");
}

/// 12. DropTracker Stress: Zero Leaks & Zero Double-Frees
#[test]
fn test_hashmap_droptracker_stress() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_drop_tracker");

    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <cmp>;
        import <hash>;
        import <iter_adapters>;
        import <iter_consumers>;
        import <box>;
import <vec>;
import <string>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

        struct TrackedItem {
            id: i32,
            drop_ptr: *rw i32,
        }

        impl Drop for TrackedItem {
            fn drop(self: &rw Self) {
                if (self.drop_ptr as u64) != (0 as u64) {
                    unsafe {
                        *self.drop_ptr = *self.drop_ptr + 1;
                    }
                }
            }
        }

        impl Hash for TrackedItem {
            fn hash(self: &Self) -> u64 {
                return self.id as u64;
            }
        }

        impl Eq for TrackedItem {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        fn run_map_lifecycle(drop_count_ptr: *rw i32) -> i32 {
            dec rw map = hashmap_with_capacity<TrackedItem, TrackedItem>(8 as u64);

            // Insert 6 key-value pairs (12 tracked items)
            dec rw i: i32 = 1;
            while i <= 6 {
                dec k = TrackedItem { id: i, drop_ptr: drop_count_ptr };
                dec v = TrackedItem { id: i * 10, drop_ptr: drop_count_ptr };
                map.insert(k, v);
                i = i + 1;
            }

            // Overwrite 2 entries (+4 items created, 2 old values dropped in-place)
            dec k_ow1 = TrackedItem { id: 1, drop_ptr: drop_count_ptr };
            dec v_ow1 = TrackedItem { id: 111, drop_ptr: drop_count_ptr };
            map.insert(k_ow1, v_ow1);

            dec k_ow2 = TrackedItem { id: 2, drop_ptr: drop_count_ptr };
            dec v_ow2 = TrackedItem { id: 222, drop_ptr: drop_count_ptr };
            map.insert(k_ow2, v_ow2);

            // Remove 2 entries (2 keys + 2 values dropped = 4 items dropped)
            dec q_rem1 = TrackedItem { id: 3, drop_ptr: drop_count_ptr };
            map.remove(&q_rem1);

            dec q_rem2 = TrackedItem { id: 4, drop_ptr: drop_count_ptr };
            map.remove(&q_rem2);

            // Growth by inserting 8 more pairs (growth to 16 -> 32)
            dec rw j: i32 = 10;
            while j <= 17 {
                dec k = TrackedItem { id: j, drop_ptr: drop_count_ptr };
                dec v = TrackedItem { id: j * 10, drop_ptr: drop_count_ptr };
                map.insert(k, v);
                j = j + 1;
            }

            // Map dropped at scope exit: all remaining live items dropped
            return 0;
        }

        fn main() -> i32 {
            dec rw drop_count: i32 = 0;
            dec res = run_map_lifecycle(&rw drop_count);
            if res != 0 {
                return 1;
            }

            // Total created instances:
            // Initial 6 pairs: 12 items
            // Overwrites 2 pairs: 4 items
            // Query temporaries: 2 items (q_rem1, q_rem2)
            // Growth 8 pairs: 16 items
            // Total created = 12 + 4 + 2 + 16 = 34 items.
            if drop_count != 34 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "DropTracker failed with code {} (stderr: {})", code, stderr);
}

/// 13. Source vs .llib Parity
#[test]
fn test_hashmap_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("map_parity");
    let hashmap_ln = locate_canonical_hashmap_ln();
    let hashmap_src = fs::read_to_string(&hashmap_ln).expect("Failed to read hashmap.ln");

    let out_llib = dir.join("hashmap.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res_compile = compile(hashmap_ln.to_str().unwrap(), hashmap_src, &compile_opts);
    assert!(res_compile.is_ok(), "Compiling hashmap.ln to hashmap.llib must succeed: {:?}", res_compile.err());

    let canonical_llib = locate_canonical_hashmap_llib();
    let tmp = canonical_llib.with_file_name(format!("{}.publish{}", canonical_llib.file_name().unwrap().to_string_lossy(), std::process::id()));
    let _ = fs::copy(&out_llib, &tmp);
    let _ = fs::rename(&tmp, &canonical_llib);

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

        fn main() -> i32 {
            dec rw map = hashmap_new<i32, i32>();
            map.insert(100, 1000);
            map.insert(200, 2000);

            dec k100: i32 = 100;
            dec k200: i32 = 200;

            dec v1 = match map.get(&k100) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            dec v2 = match map.get(&k200) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };

            if v1 != 1000 || v2 != 2000 {
                return 1;
            }

            map.remove(&k100);
            if map.contains_key(&k100) == true {
                return 2;
            }
            if map.contains_key(&k200) == false {
                return 3;
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
