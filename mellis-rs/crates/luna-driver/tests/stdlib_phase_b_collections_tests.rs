use luna_driver::CompilerOptions;
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_phase_b_collections_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn compile_and_run(test_name: &str, source: &str) -> (i32, String, String) {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir(test_name);
    let main_path = temp.join("main.ln");
    let exe_path = temp.join(format!("{}.exe", test_name));

    fs::write(&main_path, source).unwrap();

    let options = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = luna_driver::compile_and_render(main_path.to_str().unwrap(), source.to_string(), &options);
    if let Err(rendered) = &compile_res {
        eprintln!("RENDERED DIAGNOSTIC:\n{}", rendered);
    }
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());
    assert!(exe_path.is_file(), "Executable was not produced at {:?}", exe_path);

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

/// 1. Vec::insert and Vec::remove operations
#[test]
fn test_vec_insert_remove_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(30);

    // Insert at middle (index 1) -> [10, 20, 30]
    v.insert(1 as u64, 20);
    if v.len() != (3 as u64) { return 1; }
    dec v0 = match v.get(0 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec v1 = match v.get(1 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec v2 = match v.get(2 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if v0 != 10 { return 2; }
    if v1 != 20 { return 3; }
    if v2 != 30 { return 4; }

    // Insert at front (index 0) -> [5, 10, 20, 30]
    v.insert(0 as u64, 5);
    if v.len() != (4 as u64) { return 5; }
    dec f0 = match v.get(0 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec f1 = match v.get(1 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if f0 != 5 { return 6; }
    if f1 != 10 { return 7; }

    // Insert at end (index 4) -> [5, 10, 20, 30, 40]
    v.insert(4 as u64, 40);
    if v.len() != (5 as u64) { return 8; }
    dec e4 = match v.get(4 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if e4 != 40 { return 9; }

    // Remove from front (index 0) -> removes 5, leaves [10, 20, 30, 40]
    dec rem_front = v.remove(0 as u64);
    if rem_front != 5 { return 10; }
    if v.len() != (4 as u64) { return 11; }
    dec r0 = match v.get(0 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if r0 != 10 { return 12; }

    // Remove from middle (index 1) -> removes 20, leaves [10, 30, 40]
    dec rem_mid = v.remove(1 as u64);
    if rem_mid != 20 { return 13; }
    if v.len() != (3 as u64) { return 14; }
    dec r1 = match v.get(1 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if r1 != 30 { return 15; }

    // Remove from end (index 2) -> removes 40, leaves [10, 30]
    dec rem_end = v.remove(2 as u64);
    if rem_end != 40 { return 16; }
    if v.len() != (2 as u64) { return 17; }
    dec fin0 = match v.get(0 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec fin1 = match v.get(1 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if fin0 != 10 { return 18; }
    if fin1 != 30 { return 19; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_vec_insert_remove_e2e", src);
    assert_eq!(code, 0, "test_vec_insert_remove_e2e failed with code {}", code);
}

/// 2. Box::into_inner method and explicit into_inner extraction
#[test]
fn test_box_into_inner_method_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;

struct Point {
    x: i32,
    y: i32,
};

fn main() -> i32 {
    // Primitive into_inner
    dec b1 = std::box_new<i32>(42);
    dec val1 = b1.into_inner();
    if val1 != 42 { return 1; }

    // Struct into_inner
    dec b2 = std::box_new<Point>(Point { x: 100, y: 200 });
    dec pt = b2.into_inner();
    if pt.x != 100 || pt.y != 200 { return 2; }

    // Free function box_into_inner
    dec b3 = std::box_new<i32>(99);
    dec val3 = std::box_into_inner<i32>(b3);
    if val3 != 99 { return 3; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_box_into_inner_method_e2e", src);
    assert_eq!(code, 0, "test_box_into_inner_method_e2e failed with code {}", code);
}

/// 3. Composite Vec of Boxes ownership and lifecycle
#[test]
fn test_vec_box_composite_lifecycle_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <box>;
import <vec>;

struct Item {
    id: i32,
};

fn main() -> i32 {
    dec rw v = vec_new<Box<Item>>();
    v.push(std::box_new<Item>(Item { id: 10 }));
    v.push(std::box_new<Item>(Item { id: 30 }));

    // Insert boxed item at middle
    v.insert(1 as u64, std::box_new<Item>(Item { id: 20 }));
    if v.len() != (3 as u64) { return 1; }

    // Check values
    dec b0_id = match v.get(0 as u64) { Option::Some(b) -> b.as_ref().id, Option::None -> 0 };
    dec b1_id = match v.get(1 as u64) { Option::Some(b) -> b.as_ref().id, Option::None -> 0 };
    dec b2_id = match v.get(2 as u64) { Option::Some(b) -> b.as_ref().id, Option::None -> 0 };
    if b0_id != 10 { return 2; }
    if b1_id != 20 { return 3; }
    if b2_id != 30 { return 4; }

    // Remove middle box and unpack it
    dec rem_box = v.remove(1 as u64);
    dec item = rem_box.into_inner();
    if item.id != 20 { return 5; }
    if v.len() != (2 as u64) { return 6; }

    // Pop remaining
    dec last_box = v.pop().unwrap();
    if last_box.as_ref().id != 30 { return 7; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_vec_box_composite_lifecycle_e2e", src);
    assert_eq!(code, 0, "test_vec_box_composite_lifecycle_e2e failed with code {}", code);
}

/// 4. HashMap and HashSet operations and mutations
#[test]
fn test_hashmap_hashset_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <hash>;
import <cmp>;
import <hashmap>;
import <hashset>;

fn main() -> i32 {
    // 1. HashMap operations
    dec rw map = hashmap_new<i32, i32>();
    if map.is_empty() == false { return 1; }

    dec ins1 = map.insert(1, 100);
    dec ins2 = map.insert(2, 200);
    dec ins3 = map.insert(3, 300);
    if ins1 == false || ins2 == false || ins3 == false { return 2; }
    if map.len() != (3 as u64) { return 3; }

    // Overwrite
    dec ins_dup = map.insert(2, 250);
    if ins_dup == true { return 4; } // overwrite returns false
    if map.len() != (3 as u64) { return 5; }

    // Contains & Get
    dec k2 = 2;
    if map.contains_key(&k2) == false { return 6; }
    dec v2 = match map.get(&k2) { Option::Some(val) -> *val, Option::None -> 0 };
    if v2 != 250 { return 7; }

    // Mutate via get_mut
    match map.get_mut(&k2) {
        Option::Some(val_ref) -> {
            *val_ref = 280;
        },
        Option::None -> { return 8; },
    }
    dec v2_after = match map.get(&k2) { Option::Some(val) -> *val, Option::None -> 0 };
    if v2_after != 280 { return 9; }

    // Remove
    dec rem = map.remove(&k2);
    if rem == false { return 10; }
    if map.contains_key(&k2) == true { return 11; }
    if map.len() != (2 as u64) { return 12; }

    // 2. HashSet operations
    dec rw set = hashset_new<i32>();
    if set.is_empty() == false { return 13; }

    dec s_ins1 = set.insert(10);
    dec s_ins2 = set.insert(20);
    if s_ins1 == false || s_ins2 == false { return 14; }
    if set.len() != (2 as u64) { return 15; }

    // Duplicate insert
    dec s_ins_dup = set.insert(10);
    if s_ins_dup == true { return 16; } // duplicate returns false
    if set.len() != (2 as u64) { return 17; }

    // Contains & Remove
    dec s_val = 10;
    if set.contains(&s_val) == false { return 18; }
    dec s_rem = set.remove(&s_val);
    if s_rem == false { return 19; }
    if set.contains(&s_val) == true { return 20; }
    if set.len() != (1 as u64) { return 21; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_hashmap_hashset_e2e", src);
    assert_eq!(code, 0, "test_hashmap_hashset_e2e failed with code {}", code);
}

/// 5. Concrete iter_collect functions for Vec, HashSet, HashMap
#[test]
fn test_iter_collect_e2e() {
    let src = r#"
import <core/panic>;
import <mem>;
import <slice>;
import <copy>;
import <clone>;
import <ptr>;
import <hash>;
import <cmp>;
import <box>;
import <vec>;
import <hashmap>;
import <hashset>;
import <iter_collect>;

fn main() -> i32 {
    // 1. iter_collect_vec from VecIntoIter
    dec rw src_vec = vec_new<i32>();
    src_vec.push(1);
    src_vec.push(2);
    src_vec.push(3);

    dec collected_vec = iter_collect_vec(src_vec.into_iter());
    if collected_vec.len() != (3 as u64) { return 1; }
    dec cv0 = match collected_vec.get(0 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec cv1 = match collected_vec.get(1 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    dec cv2 = match collected_vec.get(2 as u64) { Option::Some(r) -> *r, Option::None -> 0 };
    if cv0 != 1 { return 2; }
    if cv1 != 2 { return 3; }
    if cv2 != 3 { return 4; }

    // 2. iter_collect_hashset from VecIntoIter
    dec rw src_vec2 = vec_new<i32>();
    src_vec2.push(10);
    src_vec2.push(20);
    src_vec2.push(10); // duplicate

    dec collected_set = iter_collect_hashset(src_vec2.into_iter());
    if collected_set.len() != (2 as u64) { return 5; }
    dec c10 = 10;
    dec c20 = 20;
    if collected_set.contains(&c10) == false { return 6; }
    if collected_set.contains(&c20) == false { return 7; }

    return 0;
}
"#;
    let (code, _, _) = compile_and_run("test_iter_collect_e2e", src);
    assert_eq!(code, 0, "test_iter_collect_e2e failed with code {}", code);
}
