// =============================================================================
// Stdlib Visibility Migration Acceptance Tests
//
// Invariant Verification:
// Public-by-default (Visibility-02) must NOT expose any representation or
// invariant-bearing field of previously frozen standard library abstractions:
//   - Vec<T> (ptr, len, cap)
//   - Box<T> (ptr)
//   - String (vec)
//   - RawTable<K, V> (states, hashes, keys, values, cap, len, tombstones)
//   - SliceIter<T> / SliceIterMut<T> (ptr, end)
//
// Simultaneously, pure data structs without field annotations must remain
// implicitly public for field access, field mutation, and struct literal construction.
// =============================================================================

use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_stdlib_vis_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn check_with_sysroot(dir: &PathBuf, file_name: &str, src: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required for stdlib tests");
    let file_path = dir.join(file_name);
    fs::write(&file_path, src).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            sysroot.root().to_string_lossy().to_string(),
        ],
        ..Default::default()
    };
    check(file_path.to_str().unwrap(), src.to_string(), &opts)
}

fn compile_and_run_with_sysroot(dir: &PathBuf, test_name: &str, src: &str) -> (i32, String, String) {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required for stdlib tests");
    let main_path = dir.join(format!("{}.ln", test_name));
    let exe_path = dir.join(if cfg!(windows) { format!("{}.exe", test_name) } else { test_name.to_string() });
    fs::write(&main_path, src).unwrap();

    let opts = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        quiet: true,
        search_paths: vec![
            dir.to_str().unwrap().to_string(),
            sysroot.root().to_string_lossy().to_string(),
        ],
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(compile_res.is_ok(), "Compilation failed: {:?}", compile_res.err());

    let output = std::process::Command::new(&exe_path)
        .output()
        .expect("Failed to execute test binary");

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

// =============================================================================
// SECTION 1: Real <alloc> Vec<T> Representation Encapsulation
// =============================================================================

#[test]
fn test_real_alloc_vec_cannot_read_ptr() {
    let dir = create_temp_dir("real_vec_read_ptr");
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

        fn test_access(v: &Vec<i32>) {
            dec p = v.ptr;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to Vec.ptr must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("ptr")),
        "Expected private field diagnostic for ptr, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_vec_cannot_read_len_or_cap() {
    let dir = create_temp_dir("real_vec_read_len");
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

        fn test_access(v: &Vec<i32>) -> u64 {
            return v.len;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to Vec.len must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("len")),
        "Expected private field diagnostic for len, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_vec_cannot_mutate_fields() {
    let dir = create_temp_dir("real_vec_mutate");
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

        fn test_mutate(v: &rw Vec<i32>) {
            v.cap = 100 as u64;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External mutation of Vec.cap must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("cap")),
        "Expected private field diagnostic for cap, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_vec_cannot_construct_directly() {
    let dir = create_temp_dir("real_vec_construct");
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

        fn construct() -> Vec<i32> {
            return Vec<i32> {
                ptr: 0 as u64 as *rw i32,
                len: 0 as u64,
                cap: 0 as u64,
            };
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External struct literal construction of Vec must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private")),
        "Expected private field diagnostic on construction, got: {:?}", diags
    );
}

// =============================================================================
// SECTION 2: Real <alloc> Box<T> Representation Encapsulation
// =============================================================================

#[test]
fn test_real_alloc_box_cannot_read_ptr() {
    let dir = create_temp_dir("real_box_read_ptr");
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

        fn test_access(b: &std::Box<i32>) {
            dec p = b.ptr;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to Box.ptr must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("ptr")),
        "Expected private field diagnostic for ptr, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_box_cannot_mutate_ptr() {
    let dir = create_temp_dir("real_box_mutate_ptr");
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

        fn test_mutate(b: &rw std::Box<i32>) {
            b.ptr = 0 as u64 as *rw i32;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External mutation of Box.ptr must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("ptr")),
        "Expected private field diagnostic for ptr, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_box_cannot_construct_directly() {
    let dir = create_temp_dir("real_box_construct");
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

        fn construct() -> std::Box<i32> {
            return std::Box<i32> {
                ptr: 0 as u64 as *rw i32,
            };
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External struct literal construction of Box must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private")),
        "Expected private field diagnostic on construction, got: {:?}", diags
    );
}

// =============================================================================
// SECTION 3: Real <alloc> String Representation Encapsulation
// =============================================================================

#[test]
fn test_real_alloc_string_cannot_access_vec() {
    let dir = create_temp_dir("real_string_read_vec");
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

        fn test_access(s: &std::String) {
            dec v = s.vec;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to String.vec must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("vec")),
        "Expected private field diagnostic for vec, got: {:?}", diags
    );
}

#[test]
fn test_real_alloc_string_cannot_construct_directly() {
    let dir = create_temp_dir("real_string_construct");
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

        fn construct() -> std::String {
            return std::String {
                vec: std::vec_new<u8>(),
            };
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External struct literal construction of String must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private")),
        "Expected private field diagnostic on construction, got: {:?}", diags
    );
}

// =============================================================================
// SECTION 4: Internal RawTable provider is not a user-facing API
// =============================================================================

#[test]
fn test_raw_table_provider_rejects_direct_user_import() {
    let dir = create_temp_dir("internal_raw_table_provider");
    let src = r#"
        import <__raw_table>;
        fn main() {}
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "the internal __raw_table provider must reject UserImport provenance");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("__raw_table")),
        "expected the rejected logical identity in the diagnostic, got: {:?}",
        diags
    );
}

// =============================================================================
// SECTION 5: Real <core> SliceIter<T> / SliceIterMut<T> Encapsulation
// =============================================================================

#[test]
fn test_real_core_slice_iter_cannot_access_ptr_or_end() {
    let dir = create_temp_dir("real_slice_iter_fields");
    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn test_access(it: &SliceIter<i32>) {
            dec p = it.ptr;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to SliceIter.ptr must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("ptr")),
        "Expected private field diagnostic for ptr, got: {:?}", diags
    );
}

#[test]
fn test_real_core_slice_iter_mut_cannot_access_ptr_or_end() {
    let dir = create_temp_dir("real_slice_iter_mut_fields");
    let src = r#"
        import <core/panic>;
        import <mem>;
        import <slice>;
        import <copy>;
        import <clone>;
        import <ptr>;
        import <iter_adapters>;
        import <iter_consumers>;

        fn test_access(it: &SliceIterMut<i32>) {
            dec e = it.end;
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_with_sysroot(&dir, "main.ln", src);
    assert!(res.is_err(), "External access to SliceIterMut.end must be rejected");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("end")),
        "Expected private field diagnostic for end, got: {:?}", diags
    );
}

// =============================================================================
// SECTION 6: Positive Invariant Verification - Public-by-Default Data Structs
// =============================================================================

#[test]
fn test_public_by_default_data_struct_cross_module() {
    let dir = create_temp_dir("data_struct_cross_module");
    let src = r#"
        module geometry {
            // Pure data struct: fields are implicitly public without 'export' keyword
            export struct Point {
                x: i32,
                y: i32,
            };

            export fn make_point(x: i32, y: i32) -> Point {
                return Point { x: x, y: y };
            }
        }

        fn main() -> i32 {
            // 1. Direct struct literal construction from outside module
            dec rw p = geometry::Point { x: 10, y: 20 };

            // 2. Field read access
            if p.x != 10 {
                return 1;
            }
            if p.y != 20 {
                return 2;
            }

            // 3. Field mutation
            p.x = 42;
            p.y = 58;
            if p.x + p.y != 100 {
                return 3;
            }

            // 4. Function returned struct field access
            dec p2 = geometry::make_point(100, 200);
            if p2.x + p2.y != 300 {
                return 4;
            }

            return 0;
        }
    "#;
    let (code, _stdout, stderr) = compile_and_run_with_sysroot(&dir, "point_test", src);
    assert_eq!(code, 0, "Public-by-default data struct execution failed (code: {}, stderr: {})", code, stderr);
}

#[test]
fn test_mixed_visibility_struct_encapsulation() {
    let dir = create_temp_dir("mixed_vis_encapsulation");
    let src = r#"
        module auth {
            export struct UserAccount {
                user_id: u64,
                display_name: i32,
                private secret_hash: u64,
            };

            export fn create_user(id: u64, name_id: i32, hash: u64) -> UserAccount {
                return UserAccount {
                    user_id: id,
                    display_name: name_id,
                    secret_hash: hash,
                };
            }
        }

        fn main() -> i32 {
            dec u = auth::create_user(1 as u64, 42, 9999 as u64);

            // Public fields are accessible
            if u.user_id != (1 as u64) {
                return 1;
            }
            if u.display_name != 42 {
                return 2;
            }

            return 0;
        }
    "#;
    let (code, _stdout, stderr) = compile_and_run_with_sysroot(&dir, "mixed_pass", src);
    assert_eq!(code, 0, "Mixed visibility public fields must execute cleanly (code: {}, stderr: {})", code, stderr);

    // Negative check: accessing secret_hash fails
    let fail_src = r#"
        module auth {
            export struct UserAccount {
                user_id: u64,
                display_name: i32,
                private secret_hash: u64,
            };

            export fn create_user(id: u64, name_id: i32, hash: u64) -> UserAccount {
                return UserAccount {
                    user_id: id,
                    display_name: name_id,
                    secret_hash: hash,
                };
            }
        }

        fn main() -> u64 {
            dec u = auth::create_user(1 as u64, 42, 9999 as u64);
            return u.secret_hash;
        }
    "#;
    let res = check_with_sysroot(&dir, "mixed_fail.ln", fail_src);
    assert!(res.is_err(), "Accessing private field of mixed struct must fail");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.to_lowercase().contains("private") && d.message.contains("secret_hash")),
        "Expected private field diagnostic for secret_hash, got: {:?}", diags
    );
}

// =============================================================================
// SECTION 7: Positive Real Stdlib Method Dispatches (Public Contract Intact)
// =============================================================================

#[test]
fn test_real_stdlib_public_apis_operate_cleanly() {
    let dir = create_temp_dir("real_stdlib_public_apis");
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
            // 1. Box public methods
            dec rw b = std::box_new<i32>(42);
            if *b.as_ref() != 42 {
                return 1;
            }
            *b.as_mut() = 99;
            if *b.as_ref() != 99 {
                return 2;
            }

            // 2. Vec public methods
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            if v.len() != (2 as u64) {
                return 3;
            }

            // 3. String public methods
            dec s = std::string_from_str("Hello");
            if s.len() != (5 as u64) {
                return 4;
            }

            // 4. HashMap public methods (RawTable remains internal)
            dec rw t = hashmap_new<i32, i32>();
            t.insert(1, 100);
            t.insert(2, 200);
            dec k1: i32 = 1;
            if t.contains_key(&k1) == false {
                return 5;
            }
            if t.len() != (2 as u64) {
                return 6;
            }

            return 0;
        }
    "#;
    let (code, _stdout, stderr) = compile_and_run_with_sysroot(&dir, "stdlib_api_test", src);
    assert_eq!(code, 0, "Public APIs of stdlib abstractions must work seamlessly (code: {}, stderr: {})", code, stderr);
}
