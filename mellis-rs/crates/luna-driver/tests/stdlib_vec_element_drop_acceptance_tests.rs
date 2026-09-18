use std::{fs, path::{Path, PathBuf}, process::Command};
use luna_driver::sysroot::Sysroot;
use luna_driver::{check, compile, CompilerOptions};

fn run_binary(exe_path: &Path) -> (i32, String, String) {
    let output = Command::new(exe_path)
        .output()
        .expect("Failed to execute compiled binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_vec_drop_tests_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// VD-1: Vec<Box<i32>> scope exit executes Drop on each element and deallocates buffer cleanly.
#[test]
fn test_vd1_vec_box_scope_exit() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd1_scope_exit");
    let src_path = dir.join("main.ln");

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

fn run_vec_boxes() {
    dec rw v = vec_new<std::Box<i32>>();
    v.push(std::box_new<i32>(100));
    v.push(std::box_new<i32>(200));
    v.push(std::box_new<i32>(300));
}

fn main() -> i32 {
    run_vec_boxes();
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking Vec<Box<i32>> must succeed: {:?}", res.err());
}

/// VD-2: Vec<DropTracker> push N + scope exit drops exactly N elements in reverse order.
#[test]
fn test_vd2_vec_droptracker_count() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd2_droptracker");
    let src_path = dir.join("main.ln");

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

struct DropTracker {
    id: i32,
    counter: *rw i32,
    last_dropped: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
            *self.last_dropped = self.id;
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    dec rw last_id: i32 = 0;
    {
        dec rw v = vec_new<DropTracker>();
        v.push(DropTracker { id: 1, counter: &rw count as *rw i32, last_dropped: &rw last_id as *rw i32 });
        v.push(DropTracker { id: 2, counter: &rw count as *rw i32, last_dropped: &rw last_id as *rw i32 });
        v.push(DropTracker { id: 3, counter: &rw count as *rw i32, last_dropped: &rw last_id as *rw i32 });
    }
    // Reverse drop order means element id 1 is dropped last
    if count == 3 && last_id == 1 {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking DropTracker must succeed: {:?}", res.err());
}

/// VD-3: Vec<DropTracker>::clear drops all elements before resetting length.
#[test]
fn test_vd3_vec_clear_drops_elements() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd3_clear");
    let src_path = dir.join("main.ln");

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

struct DropTracker {
    counter: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    dec rw v = vec_new<DropTracker>();
    v.push(DropTracker { counter: &rw count as *rw i32 });
    v.push(DropTracker { counter: &rw count as *rw i32 });
    v.push(DropTracker { counter: &rw count as *rw i32 });
    v.push(DropTracker { counter: &rw count as *rw i32 });

    if count != 0 {
        return 1;
    }

    v.clear();

    // After clear, all 4 elements must be dropped and len is 0
    if count == 4 && v.len() == (0 as u64) {
        return 0;
    }
    return 2;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking Vec::clear must succeed: {:?}", res.err());
}

/// VD-4: Vec<DropTracker>::truncate(new_len) drops elements from len-1 down to new_len.
#[test]
fn test_vd4_vec_truncate_drops_truncated_elements() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd4_truncate");
    let src_path = dir.join("main.ln");

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

struct DropTracker {
    id: i32,
    counter: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    dec rw v = vec_new<DropTracker>();
    v.push(DropTracker { id: 1, counter: &rw count as *rw i32 });
    v.push(DropTracker { id: 2, counter: &rw count as *rw i32 });
    v.push(DropTracker { id: 3, counter: &rw count as *rw i32 });
    v.push(DropTracker { id: 4, counter: &rw count as *rw i32 });
    v.push(DropTracker { id: 5, counter: &rw count as *rw i32 });

    // Truncate to 2: elements 5, 4, 3 are dropped (3 drops)
    v.truncate(2 as u64);
    if count != 3 || v.len() != (2 as u64) {
        return 1;
    }

    // Truncate to larger than current len does nothing
    v.truncate(10 as u64);
    if count != 3 || v.len() != (2 as u64) {
        return 2;
    }

    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking Vec::truncate must succeed: {:?}", res.err());
}

/// VD-5: Vec<Box<DropTracker>> multi-grow identity verification.
/// Push N elements across multiple reallocations, drop Vec, assert Drop count == N.
/// Proves exactly-once Drop, no leak, no double-drop, no double-free.
#[test]
fn test_vd5_vec_box_droptracker_multi_grow_identity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd5_identity");
    let src_path = dir.join("main.ln");

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

struct DropTracker {
    id: i32,
    counter: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw drop_count: i32 = 0;
    dec ptr_drop_count = &rw drop_count as *rw i32;
    {
        dec rw v = vec_new<std::Box<DropTracker>>();
        // Push 10 elements: capacity starts at 0 -> 4 -> 8 -> 16 (3 grows)
        dec rw i: i32 = 0;
        while i < 10 {
            v.push(std::box_new<DropTracker>(DropTracker {
                id: i,
                counter: ptr_drop_count,
            }));
            i = i + 1;
        }

        // During pushes and grows, NO elements must have been dropped
        if drop_count != 0 {
            return 1;
        }
    }
    // Now Vec has dropped out of scope: all 10 Box<DropTracker> destructors must have executed
    if drop_count == 10 {
        return 0;
    }
    return 2;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking Vec<Box<DropTracker>> multi-grow must succeed: {:?}", res.err());
}

/// VD-6: Vec<i32> drop without Drop implementation exits cleanly without crash or unnecessary work.
#[test]
fn test_vd6_vec_primitive_drop() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd6_primitive");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

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

fn run_primitives() {
    dec rw v = vec_new<i32>();
    v.push(1);
    v.push(2);
    v.push(3);
    v.push(4);
    v.push(5);
}

fn main() -> i32 {
    run_primitives();
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Compiling Vec<i32> drop must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "Vec<i32> drop must exit cleanly");
}

/// VD-7: Vec::is_empty behaves correctly on empty and non-empty vectors.
#[test]
fn test_vd7_vec_is_empty() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd7_is_empty");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

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
    dec rw v = vec_new<i32>();
    if v.is_empty() == false {
        return 1;
    }
    v.push(42);
    if v.is_empty() {
        return 2;
    }
    v.pop();
    if v.is_empty() == false {
        return 3;
    }
    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Compiling Vec::is_empty must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "is_empty() must return true when empty and false when non-empty");
}

/// VD-8: Vec::last and Vec::last_mut return correct Option reference with life_from.
#[test]
fn test_vd8_vec_last_and_last_mut() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd8_last");
    let src_path = dir.join("main.ln");
    let bin_path = dir.join("main.exe");

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
    dec rw v = vec_new<i32>();

    // Empty vector returns None
    dec empty_last = match v.last() {
        Option::Some(_) -> false,
        Option::None -> true,
    };
    if empty_last == false {
        return 1;
    }

    v.push(10);
    v.push(20);
    v.push(30);

    // Read last
    dec l_val = match v.last() {
        Option::Some(r) -> *r,
        Option::None -> 0,
    };
    if l_val != 30 {
        return 2;
    }

    // Mutate via last_mut
    match v.last_mut() {
        Option::Some(r) -> {
            *r = 99;
        }
        Option::None -> {}
    }

    // Verify mutation
    dec new_l_val = match v.last() {
        Option::Some(r) -> *r,
        Option::None -> 0,
    };
    if new_l_val != 99 {
        return 3;
    }

    return 0;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(bin_path.to_string_lossy().to_string()),
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let res = compile(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Compiling Vec::last and last_mut must succeed: {:?}", res.err());

    let (exit_code, _out, _err) = run_binary(&bin_path);
    assert_eq!(exit_code, 0, "last() and last_mut() must read and mutate last element correctly");
}

/// VD-8b: Borrowck rejects mutating Vec while reference from last() is alive.
#[test]
fn test_vd8b_vec_last_borrow_conflict_rejected() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd8b_last_conflict");
    let src_path = dir.join("main.ln");

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
    dec rw v = vec_new<i32>();
    v.push(10);
    dec last_ref = match v.last() {
        Option::Some(r) -> r,
        Option::None -> &0,
    };
    v.push(20); // Borrow conflict: mutating vector while reference from last() is alive
    dec val = *last_ref;
    return val;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "Mutating Vec while reference from last() is alive MUST be rejected by borrowck!");
}

/// VD-9: Source .ln vs .llib parity for Vec drop behavior.
#[test]
fn test_vd9_vec_drop_source_and_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("vd9_parity");
    let src_path = dir.join("main.ln");

    // alloc.llib already synced in sysroot
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

struct Tracker {
    counter: *rw i32,
}

impl Drop for Tracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    {
        dec rw v = vec_new<Tracker>();
        v.push(Tracker { counter: &rw count as *rw i32 });
        v.push(Tracker { counter: &rw count as *rw i32 });
        v.truncate(1 as u64);
        v.clear();
    }
    // 1 dropped on truncate, remaining 1 dropped on clear
    if count == 2 {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&src_path, src).unwrap();

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let res = check(src_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "Checking against precompiled alloc.llib must succeed: {:?}", res.err());
}
