use luna_driver::CompilerOptions;
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_phase4b_vec_depth_tests")
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

/// 1. Vec::swap_remove with DropProbe tracking
#[test]
fn test_vec_swap_remove_drop_probe() {
    let src = r#"
import <core/panic>;
import <vec>;

struct DropProbe {
    counter: *rw i32,
    id: i32,
};

impl Drop for DropProbe {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw drop_count: i32 = 0;
    dec probe_ptr = &rw drop_count as *rw i32;

    {
        dec rw v = vec_new<DropProbe>();
        v.push(DropProbe { counter: probe_ptr, id: 10 });
        v.push(DropProbe { counter: probe_ptr, id: 20 });
        v.push(DropProbe { counter: probe_ptr, id: 30 });
        v.push(DropProbe { counter: probe_ptr, id: 40 });

        // swap_remove at index 1 (id 20).
        // Element at index 3 (id 40) is swapped into index 1.
        dec removed = v.swap_remove(1 as u64);
        if removed.id != 20 {
            return 1;
        }
        if v.len() != (3 as u64) {
            return 2;
        }
        if v.get(1 as u64).unwrap().id != 40 {
            return 3;
        }

        // Drop 'removed' explicitly by going out of its scope
        {
            dec _dummy = removed;
        }
        // Exactly 1 drop so far
        if drop_count != 1 {
            return 4;
        }
    }

    // Now v has dropped its remaining 3 elements (10, 40, 30)
    // Total drop count must be exactly 4 (no leaks, no double drops)
    if drop_count != 4 {
        return 5;
    }

    return 0;
}
"#;
    let (code, _, err) = compile_and_run("vec_swap_remove_probe", src);
    assert_eq!(code, 0, "Test failed with code {} and stderr: {}", code, err);
}

/// 2. Vec::retain with DropProbe tracking
#[test]
fn test_vec_retain_drop_probe() {
    let src = r#"
import <core/panic>;
import <vec>;

struct Item {
    counter: *rw i32,
    id: i32,
};

impl Drop for Item {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

fn is_even(item: &Item) -> bool {
    return (item.id % 2) == 0;
}

fn main() -> i32 {
    dec rw drop_count: i32 = 0;
    dec ptr = &rw drop_count as *rw i32;

    {
        dec rw v = vec_new<Item>();
        v.push(Item { counter: ptr, id: 1 });
        v.push(Item { counter: ptr, id: 2 });
        v.push(Item { counter: ptr, id: 3 });
        v.push(Item { counter: ptr, id: 4 });
        v.push(Item { counter: ptr, id: 5 });

        // Retain only even ids (2 and 4).
        // Odd items (1, 3, 5) must be dropped immediately.
        v.retain(is_even);

        if v.len() != (2 as u64) {
            return 1;
        }
        if v.get(0 as u64).unwrap().id != 2 {
            return 2;
        }
        if v.get(1 as u64).unwrap().id != 4 {
            return 3;
        }

        // 3 items (1, 3, 5) should have been dropped during retain
        if drop_count != 3 {
            return 4;
        }
    }

    // After v drops, items 2 and 4 drop, bringing total to 5
    if drop_count != 5 {
        return 5;
    }

    return 0;
}
"#;
    let (code, _, err) = compile_and_run("vec_retain_probe", src);
    assert_eq!(code, 0, "Test failed with code {} and stderr: {}", code, err);
}

/// 3. Vec::dedup with DropProbe tracking
#[test]
fn test_vec_dedup_drop_probe() {
    let src = r#"
import <core/panic>;
import <cmp>;
import <vec>;

struct Element {
    counter: *rw i32,
    val: i32,
};

impl Drop for Element {
    fn drop(self: &rw Self) {
        unsafe {
            *self.counter = *self.counter + 1;
        }
    }
}

impl Eq for Element {
    fn eq(self: &Self, other: &Self) -> bool {
        return self.val == other.val;
    }
}

fn main() -> i32 {
    dec rw drop_count: i32 = 0;
    dec ptr = &rw drop_count as *rw i32;

    {
        dec rw v = vec_new<Element>();
        v.push(Element { counter: ptr, val: 1 });
        v.push(Element { counter: ptr, val: 1 });
        v.push(Element { counter: ptr, val: 2 });
        v.push(Element { counter: ptr, val: 3 });
        v.push(Element { counter: ptr, val: 3 });
        v.push(Element { counter: ptr, val: 3 });

        v.dedup();

        // Unique elements: 1, 2, 3 (len 3)
        if v.len() != (3 as u64) {
            return 1;
        }
        if v.get(0 as u64).unwrap().val != 1 {
            return 2;
        }
        if v.get(1 as u64).unwrap().val != 2 {
            return 3;
        }
        if v.get(2 as u64).unwrap().val != 3 {
            return 4;
        }

        // 3 duplicates dropped during dedup
        if drop_count != 3 {
            return 5;
        }
    }

    // 3 remaining elements dropped when v goes out of scope
    if drop_count != 6 {
        return 6;
    }

    return 0;
}
"#;
    let (code, _, err) = compile_and_run("vec_dedup_probe", src);
    assert_eq!(code, 0, "Test failed with code {} and stderr: {}", code, err);
}

/// 4. Vec::resize with clone and truncation
#[test]
fn test_vec_resize() {
    let src = r#"
import <core/panic>;
import <clone>;
import <vec>;

fn main() -> i32 {
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);

    // Expand to 5 with fill value 99
    v.resize(5 as u64, 99);
    if v.len() != (5 as u64) {
        return 1;
    }
    if v.get(0 as u64).unwrap() != 10 { return 2; }
    if v.get(1 as u64).unwrap() != 20 { return 3; }
    if v.get(2 as u64).unwrap() != 99 { return 4; }
    if v.get(3 as u64).unwrap() != 99 { return 5; }
    if v.get(4 as u64).unwrap() != 99 { return 6; }

    // Truncate to 1
    v.resize(1 as u64, 0);
    if v.len() != (1 as u64) {
        return 7;
    }
    if v.get(0 as u64).unwrap() != 10 {
        return 8;
    }

    return 0;
}
"#;
    let (code, _, err) = compile_and_run("vec_resize", src);
    assert_eq!(code, 0, "Test failed with code {} and stderr: {}", code, err);
}

/// 5. Vec Clone and Eq implementations
#[test]
fn test_vec_clone_and_eq() {
    let src = r#"
import <core/panic>;
import <clone>;
import <cmp>;
import <vec>;

fn main() -> i32 {
    dec rw v1 = vec_new<i32>();
    v1.push(1);
    v1.push(2);
    v1.push(3);

    dec rw v2 = v1.clone();
    if v1.len() != v2.len() {
        return 1;
    }
    if v1.eq(&v2) == false {
        return 2;
    }

    // Mutate v2 independently
    v2.push(4);
    if v1.len() == v2.len() {
        return 3;
    }
    if v1.eq(&v2) {
        return 4;
    }

    return 0;
}
"#;
    let (code, _, err) = compile_and_run("vec_clone_eq", src);
    assert_eq!(code, 0, "Test failed with code {} and stderr: {}", code, err);
}
