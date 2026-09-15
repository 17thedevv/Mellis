use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, check, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_01_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
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

    let compile_res = compile(main_path.to_str().unwrap(), source.to_string(), &options);
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

/// 1. Direct scoped generic Drop without Vec or Box.
/// Verifies that generic struct Wrapper<T> with custom Drop executes its destructor upon scope exit.
#[test]
fn test_c_gap_01_direct_scoped_generic_drop() {
    let src = r#"
import <core>;

struct Wrapper<T> {
    val: T,
    drop_counter: *rw i32,
}

impl<T> Drop for Wrapper<T> {
    fn drop(self: &rw Self) {
        unsafe {
            *self.drop_counter = *self.drop_counter + 1;
        }
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    {
        dec w = Wrapper<i32> {
            val: 42,
            drop_counter: &rw count as *rw i32,
        };
    }
    if count != 1 {
        return 101;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("direct_scoped_generic_drop", src);
    assert_eq!(code, 0, "Wrapper<T> scope exit must run drop exactly once (exit code: {}, stderr: {})", code, stderr);
}

/// 2. Composition Invariant: User Drop + Owned Field Destruction.
/// Struct Holder<T> has owned field `tracker: T` AND implements custom Drop.
/// Invariant:
/// - User Holder::drop executes FIRST.
/// - Owned field DropTracker::drop executes SECOND.
/// - Both execute EXACTLY ONCE.
#[test]
fn test_c_gap_01_custom_drop_plus_owned_field_composition() {
    let src = r#"
import <core>;

struct DropTracker {
    id: i32,
    order_seq: *rw i32,
    tracker_order: *rw i32,
    tracker_count: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.order_seq = *self.order_seq + 1;
            *self.tracker_order = *self.order_seq;
            *self.tracker_count = *self.tracker_count + 1;
        }
    }
}

struct Holder<T> {
    tracker: T,
    order_seq: *rw i32,
    holder_order: *rw i32,
    holder_count: *rw i32,
}

impl<T> Drop for Holder<T> {
    fn drop(self: &rw Self) {
        unsafe {
            *self.order_seq = *self.order_seq + 1;
            *self.holder_order = *self.order_seq;
            *self.holder_count = *self.holder_count + 1;
        }
    }
}

fn main() -> i32 {
    dec rw seq: i32 = 0;
    dec rw h_order: i32 = 0;
    dec rw h_count: i32 = 0;
    dec rw t_order: i32 = 0;
    dec rw t_count: i32 = 0;

    dec p_seq: *rw i32 = &rw seq as *rw i32;
    dec p_h_order: *rw i32 = &rw h_order as *rw i32;
    dec p_h_count: *rw i32 = &rw h_count as *rw i32;
    dec p_t_order: *rw i32 = &rw t_order as *rw i32;
    dec p_t_count: *rw i32 = &rw t_count as *rw i32;

    {
        dec h = Holder<DropTracker> {
            tracker: DropTracker {
                id: 42,
                order_seq: p_seq,
                tracker_order: p_t_order,
                tracker_count: p_t_count,
            },
            order_seq: p_seq,
            holder_order: p_h_order,
            holder_count: p_h_count,
        };
    }

    // Holder destructor must run first (order 1)
    if h_order != 1 {
        return 101;
    }
    // Holder destructor must run exactly once
    if h_count != 1 {
        return 102;
    }
    // Owned field DropTracker destructor must run second (order 2)
    if t_order != 2 {
        return 103;
    }
    // Owned field DropTracker destructor must run exactly once
    if t_count != 1 {
        return 104;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("custom_drop_plus_owned_field", src);
    assert_eq!(code, 0, "Composition of user Drop + owned field must execute in order (code: {}, stderr: {})", code, stderr);
}

/// 3. Multi-level nested generic owned fields: Outer<Inner<DropTracker>>.
/// Verifies recursive LIFO destruction across multi-level nested generic structs with custom destructors.
#[test]
fn test_c_gap_01_nested_generic_owned_fields() {
    let src = r#"
import <core>;

struct DropTracker {
    order_seq: *rw i32,
    order: *rw i32,
    count: *rw i32,
}

impl Drop for DropTracker {
    fn drop(self: &rw Self) {
        unsafe {
            *self.order_seq = *self.order_seq + 1;
            *self.order = *self.order_seq;
            *self.count = *self.count + 1;
        }
    }
}

struct Inner<T> {
    item: T,
    order_seq: *rw i32,
    order: *rw i32,
    count: *rw i32,
}

impl<T> Drop for Inner<T> {
    fn drop(self: &rw Self) {
        unsafe {
            *self.order_seq = *self.order_seq + 1;
            *self.order = *self.order_seq;
            *self.count = *self.count + 1;
        }
    }
}

struct Outer<T> {
    inner: T,
    order_seq: *rw i32,
    order: *rw i32,
    count: *rw i32,
}

impl<T> Drop for Outer<T> {
    fn drop(self: &rw Self) {
        unsafe {
            *self.order_seq = *self.order_seq + 1;
            *self.order = *self.order_seq;
            *self.count = *self.count + 1;
        }
    }
}

fn main() -> i32 {
    dec rw seq: i32 = 0;
    dec rw o_order: i32 = 0;
    dec rw o_count: i32 = 0;
    dec rw i_order: i32 = 0;
    dec rw i_count: i32 = 0;
    dec rw t_order: i32 = 0;
    dec rw t_count: i32 = 0;

    dec p_seq: *rw i32 = &rw seq as *rw i32;
    dec p_o_order: *rw i32 = &rw o_order as *rw i32;
    dec p_o_count: *rw i32 = &rw o_count as *rw i32;
    dec p_i_order: *rw i32 = &rw i_order as *rw i32;
    dec p_i_count: *rw i32 = &rw i_count as *rw i32;
    dec p_t_order: *rw i32 = &rw t_order as *rw i32;
    dec p_t_count: *rw i32 = &rw t_count as *rw i32;

    {
        dec o = Outer<Inner<DropTracker>> {
            inner: Inner<DropTracker> {
                item: DropTracker {
                    order_seq: p_seq,
                    order: p_t_order,
                    count: p_t_count,
                },
                order_seq: p_seq,
                order: p_i_order,
                count: p_i_count,
            },
            order_seq: p_seq,
            order: p_o_order,
            count: p_o_count,
        };
    }

    // Outer drop runs 1st
    if o_order != 1 || o_count != 1 {
        return 101;
    }
    // Inner drop runs 2nd
    if i_order != 2 || i_count != 1 {
        return 102;
    }
    // Leaf DropTracker drop runs 3rd
    if t_order != 3 || t_count != 1 {
        return 103;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("nested_generic_owned_fields", src);
    assert_eq!(code, 0, "Nested generic owned fields must execute LIFO (code: {}, stderr: {})", code, stderr);
}

/// 4. Vec<Box<DropTracker>> multi-grow and dealloc.
/// Pushes 5 elements to force multiple buffer reallocations, then clears.
/// Verifies exactly 5 destructor runs and clean buffer handling.
#[test]
fn test_c_gap_01_vec_box_drop_tracker_multi_grow() {
    let src = r#"
import <core>;
import <alloc>;

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
    {
        dec rw v = vec_new<std::Box<DropTracker>>();
        v.push(std::box_new<DropTracker>(DropTracker { id: 1, counter: &rw count as *rw i32 }));
        v.push(std::box_new<DropTracker>(DropTracker { id: 2, counter: &rw count as *rw i32 }));
        v.push(std::box_new<DropTracker>(DropTracker { id: 3, counter: &rw count as *rw i32 }));
        v.push(std::box_new<DropTracker>(DropTracker { id: 4, counter: &rw count as *rw i32 }));
        v.push(std::box_new<DropTracker>(DropTracker { id: 5, counter: &rw count as *rw i32 }));
        v.clear();
    }
    if count != 5 {
        return 101;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("vec_box_droptracker_multi_grow", src);
    assert_eq!(code, 0, "Vec<Box<DropTracker>> multi-grow + clear must drop exactly 5 elements (code: {}, stderr: {})", code, stderr);
}

/// 5. Vec<Box<Box<DropTracker>>> 3-level recursive generic drop chain.
#[test]
fn test_c_gap_01_nested_box_box_drop_tracker() {
    let src = r#"
import <core>;
import <alloc>;

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
    {
        dec rw v = vec_new<std::Box<std::Box<DropTracker>>>();
        v.push(std::box_new<std::Box<DropTracker>>(std::box_new<DropTracker>(DropTracker { id: 1, counter: &rw count as *rw i32 })));
        v.push(std::box_new<std::Box<DropTracker>>(std::box_new<DropTracker>(DropTracker { id: 2, counter: &rw count as *rw i32 })));
        v.push(std::box_new<std::Box<DropTracker>>(std::box_new<DropTracker>(DropTracker { id: 3, counter: &rw count as *rw i32 })));
    }
    if count != 3 {
        return 101;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("nested_box_box_drop_tracker", src);
    assert_eq!(code, 0, "Vec<Box<Box<DropTracker>>> scope exit must drop exactly 3 elements through 3-level chain (code: {}, stderr: {})", code, stderr);
}

/// 6. Non-Drop type T emits zero drop glue and executes without drop calls.
#[test]
fn test_c_gap_01_non_drop_type_no_destructor() {
    let src = r#"
import <core>;
import <alloc>;

struct PlainData<T> {
    val: T,
}

fn main() -> i32 {
    dec p = PlainData<i32> { val: 12345 };
    dec rw v = vec_new<i32>();
    v.push(10);
    v.push(20);
    v.push(30);
    if v.len() != (3 as u64) {
        return 101;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("non_drop_type_no_destructor", src);
    assert_eq!(code, 0, "Non-Drop types must compile cleanly and produce zero destructor calls (code: {}, stderr: {})", code, stderr);
}

/// 7. Source vs .llib Parity: Vec<Box<DropTracker>> against source <alloc> and precompiled alloc.llib.
#[test]
fn test_c_gap_01_source_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let alloc_llib = sysroot.root().join("libs").join("external").join("alloc.llib");
    assert!(alloc_llib.exists(), "alloc.llib must exist for parity test");

    let src = r#"
import <core>;
import <alloc>;

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
    {
        dec rw v = vec_new<std::Box<DropTracker>>();
        v.push(std::box_new<DropTracker>(DropTracker { id: 10, counter: &rw count as *rw i32 }));
        v.push(std::box_new<DropTracker>(DropTracker { id: 20, counter: &rw count as *rw i32 }));
    }
    if count != 2 {
        return 101;
    }
    return 0;
}
"#;
    let (code, stdout, stderr) = compile_and_run("source_llib_parity", src);
    assert_eq!(code, 0, "Vec<Box<DropTracker>> with precompiled alloc.llib parity must succeed (code: {}, stderr: {})", code, stderr);
}
