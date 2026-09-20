// =============================================================================
// Stdlib Phase 04.5 HashSet<T> Acceptance Tests
//
// Verifies:
//   1. Empty Lifecycle (default constructor, capacity, empty state, clean drop)
//   2. Insert and Contains Membership
//   3. Duplicate Insert Semantics (returns false, length unchanged, item dropped)
//   4. Removal Semantics (returns true when present, false when absent, length decremented)
//   5. Clear Semantics (drops all elements, resets length to 0, capacity preserved)
//   6. Multiple Collisions Transparency
//   7. Dynamic Growth & Relocation
//   8. DropTracker Stress Test: Zero Leaks & Zero Double-Frees
//   9. Source vs .llib Parity
// =============================================================================

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_hashset_{}_{}_{}",
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


fn locate_canonical_hashset_ln() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("hashset.ln");
        if p.exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/hashset.ln");
}

fn locate_canonical_hashset_llib() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("hashset.llib");
        if p.parent().unwrap().exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/hashset.llib");
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

/// 1. Empty Lifecycle
#[test]
fn test_hashset_empty_lifecycle() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_empty_lifecycle");

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
            dec rw set = hashset_new<i32>();
            if set.len() != (0 as u64) {
                return 1;
            }
            if set.is_empty() == false {
                return 2;
            }

            dec rw set_cap = hashset_with_capacity<i32>(16 as u64);
            if set_cap.capacity() != (16 as u64) {
                return 3;
            }
            if set_cap.len() != (0 as u64) {
                return 4;
            }
            if set_cap.is_empty() == false {
                return 5;
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

/// 2. Basic Insert and Contains
#[test]
fn test_hashset_insert_and_contains() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_insert_contains");

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
            dec rw set = hashset_with_capacity<i32>(8 as u64);

            dec ins10 = set.insert(10);
            dec ins20 = set.insert(20);
            dec ins30 = set.insert(30);

            if ins10 == false || ins20 == false || ins30 == false {
                return 1;
            }
            if set.len() != (3 as u64) {
                return 2;
            }
            if set.is_empty() == true {
                return 3;
            }

            dec k10: i32 = 10;
            dec k20: i32 = 20;
            dec k30: i32 = 30;
            dec k99: i32 = 99;

            if set.contains(&k10) == false {
                return 4;
            }
            if set.contains(&k20) == false {
                return 5;
            }
            if set.contains(&k30) == false {
                return 6;
            }
            if set.contains(&k99) == true {
                return 7;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Insert and contains failed with code {} (stderr: {})", code, stderr);
}

/// 3. Duplicate Insert Semantics (returns false, length unchanged)
#[test]
fn test_hashset_duplicate_insert() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_duplicate_insert");

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
            dec rw set = hashset_with_capacity<i32>(8 as u64);

            dec first_ins = set.insert(42);
            if first_ins == false {
                return 1;
            }
            if set.len() != (1 as u64) {
                return 2;
            }

            dec second_ins = set.insert(42);
            if second_ins == true {
                return 3;
            }
            if set.len() != (1 as u64) {
                return 4;
            }

            dec third_ins = set.insert(42);
            if third_ins == true {
                return 5;
            }
            if set.len() != (1 as u64) {
                return 6;
            }

            dec k42: i32 = 42;
            if set.contains(&k42) == false {
                return 7;
            }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts)
        .expect("Execution must succeed");

    assert_eq!(code, 0, "Duplicate insert failed with code {} (stderr: {})", code, stderr);
}

/// 4. Removal Semantics
#[test]
fn test_hashset_remove() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_remove");

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
            dec rw set = hashset_with_capacity<i32>(8 as u64);
            set.insert(100);
            set.insert(200);
            set.insert(300);

            dec k100: i32 = 100;
            dec k200: i32 = 200;
            dec k300: i32 = 300;
            dec k999: i32 = 999;

            // Remove non-existent element -> returns false
            dec rem_absent = set.remove(&k999);
            if rem_absent == true {
                return 1;
            }
            if set.len() != (3 as u64) {
                return 2;
            }

            // Remove 200 -> returns true
            dec rem_200 = set.remove(&k200);
            if rem_200 == false {
                return 3;
            }
            if set.len() != (2 as u64) {
                return 4;
            }
            if set.contains(&k200) == true {
                return 5;
            }
            if set.contains(&k100) == false {
                return 6;
            }
            if set.contains(&k300) == false {
                return 7;
            }

            // Removing 200 again -> returns false
            dec rem_200_again = set.remove(&k200);
            if rem_200_again == true {
                return 8;
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

/// 5. Clear Semantics
#[test]
fn test_hashset_clear() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_clear");

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
            dec rw set = hashset_with_capacity<i32>(8 as u64);
            set.insert(1);
            set.insert(2);
            set.insert(3);

            if set.len() != (3 as u64) {
                return 1;
            }

            set.clear();

            if set.len() != (0 as u64) {
                return 2;
            }
            if set.is_empty() == false {
                return 3;
            }
            if set.capacity() != (8 as u64) {
                return 4;
            }

            dec k1: i32 = 1;
            dec k2: i32 = 2;
            dec k3: i32 = 3;

            if set.contains(&k1) == true || set.contains(&k2) == true || set.contains(&k3) == true {
                return 5;
            }

            // Can insert again after clear
            set.insert(99);
            dec k99: i32 = 99;
            if set.contains(&k99) == false {
                return 6;
            }
            if set.len() != (1 as u64) {
                return 7;
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

/// 6. Multiple Hash Collisions Transparency
#[test]
fn test_hashset_multiple_collisions() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_collisions");

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
            id: i32,
            fixed_hash: u64,
        };

        impl Hash for CollidingKey {
            fn hash(self: &Self) -> u64 {
                return self.fixed_hash;
            }
        }

        impl Eq for CollidingKey {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        fn main() -> i32 {
            dec rw set = hashset_with_capacity<CollidingKey>(8 as u64);

            // Three distinct keys that all produce hash = 3
            dec k1 = CollidingKey { id: 10, fixed_hash: 3 as u64 };
            dec k2 = CollidingKey { id: 20, fixed_hash: 3 as u64 };
            dec k3 = CollidingKey { id: 30, fixed_hash: 3 as u64 };

            set.insert(k1);
            set.insert(k2);
            set.insert(k3);

            if set.len() != (3 as u64) {
                return 1;
            }

            dec q1 = CollidingKey { id: 10, fixed_hash: 3 as u64 };
            dec q2 = CollidingKey { id: 20, fixed_hash: 3 as u64 };
            dec q3 = CollidingKey { id: 30, fixed_hash: 3 as u64 };
            dec q_missing = CollidingKey { id: 40, fixed_hash: 3 as u64 };

            if set.contains(&q1) == false || set.contains(&q2) == false || set.contains(&q3) == false {
                return 2;
            }
            if set.contains(&q_missing) == true {
                return 3;
            }

            // Remove middle key in collision probe chain
            set.remove(&q2);
            if set.len() != (2 as u64) {
                return 4;
            }
            if set.contains(&q2) == true {
                return 5;
            }
            // Probing must skip tombstone and find q3
            if set.contains(&q3) == false {
                return 6;
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

/// 7. Dynamic Growth & Relocation
#[test]
fn test_hashset_dynamic_growth() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_growth");

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
            dec rw set = hashset_with_capacity<i32>(8 as u64);

            // Insert 25 elements, forcing multiple resizes (8 -> 16 -> 32 -> 64)
            dec rw i: i32 = 1;
            while i <= 25 {
                set.insert(i);
                i = i + 1;
            }

            if set.len() != (25 as u64) {
                return 1;
            }
            if set.capacity() < (32 as u64) {
                return 2;
            }

            // Verify all 25 elements intact
            dec rw j: i32 = 1;
            while j <= 25 {
                if set.contains(&j) == false {
                    return 3;
                }
                j = j + 1;
            }

            dec absent: i32 = 999;
            if set.contains(&absent) == true {
                return 4;
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

/// 8. DropTracker Stress Test: Zero Leaks & Zero Double-Frees
#[test]
fn test_hashset_droptracker_stress() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_drop_tracker");

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

        struct TrackedElement {
            id: i32,
            drop_ptr: *rw i32,
        };

        impl Drop for TrackedElement {
            fn drop(self: &rw Self) {
                if (self.drop_ptr as u64) != (0 as u64) {
                    unsafe {
                        *self.drop_ptr = *self.drop_ptr + 1;
                    }
                }
            }
        }

        impl Hash for TrackedElement {
            fn hash(self: &Self) -> u64 {
                return self.id as u64;
            }
        }

        impl Eq for TrackedElement {
            fn eq(self: &Self, other: &Self) -> bool {
                return self.id == other.id;
            }
        }

        fn run_set_lifecycle(drop_count_ptr: *rw i32) -> i32 {
            dec rw set = hashset_with_capacity<TrackedElement>(8 as u64);

            // Insert 6 elements (6 tracked elements created)
            dec rw i: i32 = 1;
            while i <= 6 {
                dec el = TrackedElement { id: i, drop_ptr: drop_count_ptr };
                set.insert(el);
                i = i + 1;
            }

            // Attempt duplicate insertion of 2 existing elements (+2 elements created)
            // Both duplicate attempts return false and are dropped upon leaving insert scope
            dec dup1 = TrackedElement { id: 1, drop_ptr: drop_count_ptr };
            dec ins_dup1 = set.insert(dup1);
            if ins_dup1 == true {
                return 10;
            }

            dec dup2 = TrackedElement { id: 2, drop_ptr: drop_count_ptr };
            dec ins_dup2 = set.insert(dup2);
            if ins_dup2 == true {
                return 11;
            }

            // Remove 2 entries (2 query elements created: q_rem1, q_rem2)
            // In remove, 2 table elements are dropped in-place
            dec q_rem1 = TrackedElement { id: 3, drop_ptr: drop_count_ptr };
            dec rem1 = set.remove(&q_rem1);
            if rem1 == false {
                return 12;
            }

            dec q_rem2 = TrackedElement { id: 4, drop_ptr: drop_count_ptr };
            dec rem2 = set.remove(&q_rem2);
            if rem2 == false {
                return 13;
            }

            // Growth: insert 8 more elements (id 10 to 17) (+8 elements created)
            // Table grows 8 -> 16 -> 32
            dec rw j: i32 = 10;
            while j <= 17 {
                dec el = TrackedElement { id: j, drop_ptr: drop_count_ptr };
                set.insert(el);
                j = j + 1;
            }

            // Set dropped at scope exit: all 12 remaining live elements dropped
            return 0;
        }

        fn main() -> i32 {
            dec rw drop_count: i32 = 0;
            dec res = run_set_lifecycle(&rw drop_count);
            if res != 0 {
                return res;
            }

            // Total created instances:
            // Initial: 6
            // Duplicates: 2
            // Query temporaries: 2 (q_rem1, q_rem2)
            // Growth: 8
            // Total created = 6 + 2 + 2 + 8 = 18 elements.
            if drop_count != 18 {
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

/// 9. Source vs .llib Parity
#[test]
fn test_hashset_source_vs_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("set_parity");
    let hashset_ln = locate_canonical_hashset_ln();
    let hashset_src = fs::read_to_string(&hashset_ln).expect("Failed to read hashset.ln");

    let out_llib = dir.join("hashset.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res_compile = compile(hashset_ln.to_str().unwrap(), hashset_src, &compile_opts);
    assert!(res_compile.is_ok(), "Compiling hashset.ln to hashset.llib must succeed: {:?}", res_compile.err());

    let canonical_llib = locate_canonical_hashset_llib();
    let _ = fs::copy(&out_llib, &canonical_llib);

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
            dec rw set = hashset_new<i32>();
            set.insert(100);
            set.insert(200);

            dec k100: i32 = 100;
            dec k200: i32 = 200;
            dec k300: i32 = 300;

            if set.contains(&k100) == false || set.contains(&k200) == false {
                return 1;
            }
            if set.contains(&k300) == true {
                return 2;
            }

            dec rem = set.remove(&k100);
            if rem == false {
                return 3;
            }
            if set.contains(&k100) == true {
                return 4;
            }
            if set.contains(&k200) == false {
                return 5;
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

/// 10. Visibility-02: External access to private `map` field must be rejected
#[test]
fn test_hashset_private_field_access_rejected() {
    let dir = create_temp_dir("set_private_field");
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
            dec rw set = hashset_new<i32>();
            dec m = set.map; // ERROR: field `map` is private
            return 0;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(res.is_err(), "Accessing private `map` field of HashSet must be rejected by compiler");
    let diags = res.unwrap_err();
    let has_vis_err = diags.iter().any(|d| d.message.contains("private") || d.message.contains("cannot access"));
    assert!(has_vis_err, "Diagnostic must indicate visibility error: {:?}", diags);
}

/// 11. Visibility-02: External struct literal construction must be rejected
#[test]
fn test_hashset_struct_literal_construction_rejected() {
    let dir = create_temp_dir("set_struct_literal");
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
            dec rw m = hashmap_new<i32, ()>();
            dec set = std::HashSet<i32> { map: m }; // ERROR: cannot construct struct with private fields
            return 0;
        }
    "#;

    let res = check_source_with_sysroot(&dir, src);
    assert!(res.is_err(), "Direct construction of HashSet via struct literal must be rejected by compiler");
    let diags = res.unwrap_err();
    let has_vis_err = diags.iter().any(|d| d.message.contains("private") || d.message.contains("cannot construct"));
    assert!(has_vis_err, "Diagnostic must indicate private field construction error: {:?}", diags);
}

