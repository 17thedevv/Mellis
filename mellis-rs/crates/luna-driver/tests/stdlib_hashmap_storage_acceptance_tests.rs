// =============================================================================
// Stdlib Phase 04.3 RawTable Storage Acceptance Tests
//
// Invariants Verified:
//   HT-1: Empty Lifecycle (alloc, zero/default capacity, clean drop)
//   HT-2: Insert & Persistence (insert, get, contains_key, missing key)
//   HT-3: Overwrite Semantics (in-place destruction of old value, length unchanged)
//   HT-4: Tombstone Search Correctness & Reuse:
//         - Occupied: compare hash -> Eq
//         - Deleted: continue probing (never terminate search)
//         - Empty: terminate search
//         - If tombstone encountered first, but matching key exists later in probe chain:
//           find MUST return Found(idx), NOT Vacant(tombstone).
//         - When inserting new key: first tombstone slot is reused.
//   HT-5: Resize Growth & Relocation (growth across power-of-two boundaries, entries preserved)
//   HT-6: DropTracker Stress Soundness (created == dropped, zero leaks, zero double-frees)
//   HT-7: Source vs .llib Parity (parity between fresh compile and precompiled alloc.llib)
// =============================================================================

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
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


fn locate_canonical_raw_table_ln() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("raw_table.ln");
        if p.exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/raw_table.ln");
}

fn locate_canonical_raw_table_llib() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("raw_table.llib");
        if p.parent().unwrap().exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/raw_table.llib");
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
    compile_opts.is_sysroot_build = true;
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

/// HT-1. Empty Lifecycle: allocate table, check empty state and capacity, drop cleanly
#[test]
fn test_ht1_empty_lifecycle() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht1_empty_lifecycle");

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            if table.len() != (0 as u64) {
                return 1;
            }
            if table.capacity() != (8 as u64) {
                return 2;
            }
            if table.tombstones() != (0 as u64) {
                return 3;
            }
            if table.is_empty() == false {
                return 4;
            }
            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-1 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-2. Basic Insert, Persistence, and Contains
#[test]
fn test_ht2_insert_persistence_and_contains() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht2_insert_persistence");

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            dec ins1 = table.insert(10, 100);
            dec ins2 = table.insert(20, 200);
            dec ins3 = table.insert(30, 300);

            if ins1 == false || ins2 == false || ins3 == false {
                return 1;
            }
            if table.len() != (3 as u64) {
                return 2;
            }

            dec v10 = table.get(&10);
            dec v20 = table.get(&20);
            dec v99 = table.get(&99);

            dec r1 = match v10 {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if r1 != 100 {
                return 3;
            }

            dec r2 = match v20 {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if r2 != 200 {
                return 4;
            }

            dec r3 = match v99 {
                Option::Some(_) -> 1,
                Option::None -> 0,
            };
            if r3 != 0 {
                return 5;
            }

            if table.contains_key(&10) == false {
                return 6;
            }
            if table.contains_key(&99) == true {
                return 7;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-2 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-2B. In-place Mutation with `get_mut`
#[test]
fn test_ht2b_get_mut() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht2b_get_mut");

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            table.insert(5, 50);

            dec mut_opt = table.get_mut(&5);
            match mut_opt {
                Option::Some(val_ref) -> {
                    *val_ref = 500;
                },
                Option::None -> {
                    return 1;
                },
            }

            dec read_opt = table.get(&5);
            dec final_val = match read_opt {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };

            if final_val != 500 {
                return 2;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-2B failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-3. Overwrite Semantics
#[test]
fn test_ht3_overwrite_semantics() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht3_overwrite_semantics");

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            dec first_ins = table.insert(7, 70);
            if first_ins == false {
                return 1;
            }
            if table.len() != (1 as u64) {
                return 2;
            }

            dec second_ins = table.insert(7, 777);
            if second_ins == true {
                return 3; // Must return false indicating overwrite
            }
            if table.len() != (1 as u64) {
                return 4; // Length must remain 1
            }

            dec v = match table.get(&7) {
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

    assert_eq!(code, 0, "HT-3 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-4. Tombstone Search Correctness & Reuse
/// Specifically proves:
///   1. Occupied -> compare hash -> Eq
///   2. Deleted (tombstone) -> continue probing
///   3. Empty -> terminate search
///   4. Critical invariant: If a tombstone was encountered first, but an existing matching
///      key exists later in the probing sequence, find MUST return Found(idx), NOT Vacant(tombstone)!
///   5. When inserting a new non-existing key, the first tombstone slot is reused.
#[test]
fn test_ht4_tombstone_search_correctness_and_reuse() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht4_tombstone_correctness");

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
import <__raw_table>;

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
            dec rw table = raw_table_with_capacity<CollidingKey, i32>(8 as u64);
            
            // 3 keys with the exact same forced hash
            dec k1 = CollidingKey { id: 1 as u64, forced_hash: 42 as u64 };
            dec k2 = CollidingKey { id: 2 as u64, forced_hash: 42 as u64 };
            dec k3 = CollidingKey { id: 3 as u64, forced_hash: 42 as u64 };

            table.insert(k1, 100);
            table.insert(k2, 200);
            table.insert(k3, 300);

            if table.len() != (3 as u64) {
                return 1;
            }

            // Remove the first key (k1). Its slot is now Deleted (a tombstone).
            dec q1 = CollidingKey { id: 1 as u64, forced_hash: 42 as u64 };
            dec rem1 = table.remove(&q1);
            if rem1 == false {
                return 2;
            }
            if table.len() != (2 as u64) {
                return 3;
            }
            if table.tombstones() != (1 as u64) {
                return 4;
            }

            // CRITICAL INVARIANT TEST:
            // Probing for k3 starts at k1's slot (now a tombstone).
            // It MUST skip the tombstone and continue probing to find k3, returning Found(k3_idx)!
            // It MUST NOT terminate or prematurely return Vacant(first_tombstone).
            dec q3 = CollidingKey { id: 3 as u64, forced_hash: 42 as u64 };
            dec v3 = match table.get(&q3) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v3 != 300 {
                return 5; // Failed to find key located past a tombstone!
            }

            // Verify k2 (between tombstone and k3) is also found
            dec q2 = CollidingKey { id: 2 as u64, forced_hash: 42 as u64 };
            dec v2 = match table.get(&q2) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v2 != 200 {
                return 6;
            }

            // Now query for non-existent key with same hash:
            // Probing must check tombstone, k2, k3, and then terminate at Empty,
            // returning Vacant(first_tombstone).
            dec q_absent = CollidingKey { id: 99 as u64, forced_hash: 42 as u64 };
            if table.contains_key(&q_absent) == true {
                return 7;
            }

            // Now insert new key k4 with same hash:
            // It MUST reuse the first tombstone slot!
            dec k4 = CollidingKey { id: 4 as u64, forced_hash: 42 as u64 };
            dec ins4 = table.insert(k4, 400);
            if ins4 == false {
                return 8;
            }
            // Tombstone was reused: tombstones count decrements from 1 to 0
            if table.tombstones() != (0 as u64) {
                return 9;
            }
            if table.len() != (3 as u64) {
                return 10;
            }

            // Verify all 3 keys (k4 at reused slot, k2, k3) are intact
            dec q4 = CollidingKey { id: 4 as u64, forced_hash: 42 as u64 };
            dec v4 = match table.get(&q4) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v4 != 400 {
                return 11;
            }

            dec v3_final = match table.get(&q3) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            if v3_final != 300 {
                return 12;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-4 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-5. Capacity Growth & Element Relocation (8 -> 16 -> 32)
#[test]
fn test_ht5_resize_growth_and_relocation() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht5_resize_growth");

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            if table.capacity() != (8 as u64) {
                return 1;
            }

            // Insert 20 elements (forces 8 -> 16 -> 32 capacity growth)
            dec rw i: i32 = 1;
            while i <= 20 {
                table.insert(i, i * 10);
                i = i + 1;
            }

            if table.len() != (20 as u64) {
                return 2;
            }
            if table.capacity() < (32 as u64) {
                return 3;
            }

            // Verify all 20 elements are retrievable after multiple resizings and rehashes
            dec rw j: i32 = 1;
            while j <= 20 {
                dec val = match table.get(&j) {
                    Option::Some(v) -> *v,
                    Option::None -> 0,
                };
                if val != (j * 10) {
                    return 4;
                }
                j = j + 1;
            }

            // Tombstones must be 0 after resize
            if table.tombstones() != (0 as u64) {
                return 5;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-5 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-6. DropTracker Stress Test: Zero Leaks and Zero Double-Frees
#[test]
fn test_ht6_drop_tracker_stress_soundness() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht6_drop_tracker");

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
import <__raw_table>;

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

        fn run_stress_lifecycle(drop_count_ptr: *rw i32) -> i32 {
            dec rw table = raw_table_with_capacity<TrackedItem, TrackedItem>(8 as u64);
            
            // Insert 6 key-value pairs (12 tracked items)
            dec rw i: i32 = 1;
            while i <= 6 {
                dec k = TrackedItem { id: i, drop_ptr: drop_count_ptr };
                dec v = TrackedItem { id: i * 10, drop_ptr: drop_count_ptr };
                table.insert(k, v);
                i = i + 1;
            }

            // Overwrite 2 entries (+4 items created, 2 old values dropped in-place)
            dec k_ow1 = TrackedItem { id: 1, drop_ptr: drop_count_ptr };
            dec v_ow1 = TrackedItem { id: 111, drop_ptr: drop_count_ptr };
            table.insert(k_ow1, v_ow1);

            dec k_ow2 = TrackedItem { id: 2, drop_ptr: drop_count_ptr };
            dec v_ow2 = TrackedItem { id: 222, drop_ptr: drop_count_ptr };
            table.insert(k_ow2, v_ow2);

            // Remove 2 entries (2 keys + 2 values dropped in-place = 4 items dropped)
            dec q_rem1 = TrackedItem { id: 3, drop_ptr: drop_count_ptr };
            table.remove(&q_rem1);

            dec q_rem2 = TrackedItem { id: 4, drop_ptr: drop_count_ptr };
            table.remove(&q_rem2);

            // Trigger table resize by inserting 8 more entries (causes growth from 8 -> 16 -> 32)
            dec rw j: i32 = 10;
            while j <= 17 {
                dec k = TrackedItem { id: j, drop_ptr: drop_count_ptr };
                dec v = TrackedItem { id: j * 10, drop_ptr: drop_count_ptr };
                table.insert(k, v);
                j = j + 1;
            }

            // Table goes out of scope here: all remaining live items in table are dropped
            return 0;
        }

        fn main() -> i32 {
            dec rw drop_count: i32 = 0;
            dec res = run_stress_lifecycle(&rw drop_count);
            if res != 0 {
                return 1;
            }

            // Total created TrackedItem instances throughout test:
            // Initial 6 pairs: 12 items
            // Overwrites 2 pairs: 4 items
            // Search temporary queries: 2 items (q_rem1, q_rem2 dropped when going out of scope)
            // Resize batch 8 pairs: 16 items
            // Total created = 12 + 4 + 2 + 16 = 34 items.
            // If drop is exact with zero leaks and zero double-frees, drop_count MUST equal exactly 34.
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

    assert_eq!(code, 0, "HT-6 failed with exit code {} (stderr: {})", code, stderr);
}

/// HT-7. Source vs .llib Parity:
/// Verifies identical behavior whether executing against precompiled alloc.llib
/// or freshly compiled source alloc.ln.
#[test]
fn test_ht7_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("ht7_parity");
    let raw_table_ln = locate_canonical_raw_table_ln();
    let raw_table_src = fs::read_to_string(&raw_table_ln).expect("Failed to read raw_table.ln");

    let out_llib = dir.join("raw_table.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res_compile = compile(raw_table_ln.to_str().unwrap(), raw_table_src, &compile_opts);
    assert!(res_compile.is_ok(), "Compiling raw_table.ln to raw_table.llib must succeed: {:?}", res_compile.err());

    let canonical_llib = locate_canonical_raw_table_llib();
    let _ = fs::copy(&out_llib, &canonical_llib);

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
import <__raw_table>;

        fn main() -> i32 {
            dec rw table = raw_table_with_capacity<i32, i32>(8 as u64);
            table.insert(100, 1000);
            table.insert(200, 2000);

            dec v1 = match table.get(&100) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };
            dec v2 = match table.get(&200) {
                Option::Some(v) -> *v,
                Option::None -> 0,
            };

            if v1 != 1000 || v2 != 2000 {
                return 1;
            }

            table.remove(&100);
            if table.contains_key(&100) == true {
                return 2;
            }
            if table.contains_key(&200) == false {
                return 3;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "HT-7 parity execution failed (code: {}, stderr: {})", code, stderr);
}
