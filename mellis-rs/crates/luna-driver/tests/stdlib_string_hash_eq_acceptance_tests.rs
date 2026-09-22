use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_string_hash_eq_{}", name));
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

/// STDLIB-GAP-04: Test 1
/// Verifies String content equality across different capacities, constructors, and multibyte UTF-8.
#[test]
fn test_string_eq_semantics() {
    let src = r#"
import <string>;
import <cmp>;

fn main() -> i32 {
    // 1. Empty strings with different capacities
    dec s1 = string_new();
    dec s2 = string_with_capacity(64 as u64);
    if s1.eq_string(&s2) == false {
        return 1;
    }

    // 2. ASCII strings with same content from different constructors
    dec s3 = string_from_str("hello");
    dec rw s4 = string_new();
    s4.push_str("hello");
    if s3.eq_string(&s4) == false {
        return 2;
    }

    // 3. Different content must not be equal
    dec s5 = string_from_str("world");
    if s3.eq_string(&s5) == true {
        return 3;
    }

    // 4. Multibyte UTF-8 strings
    dec s6 = string_from_str("Xin chào");
    dec s7 = string_from_str("Xin chào");
    if s6.eq_string(&s7) == false {
        return 4;
    }

    // 5. Different lengths must not be equal
    dec s8 = string_from_str("hell");
    if s3.eq_string(&s8) == true {
        return 5;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_string_eq", src);
    assert_eq!(code, 0, "String eq test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-04: Test 2
/// Verifies Hash invariant: a.eq_string(b) => a.hash_string() == b.hash_string().
#[test]
fn test_string_hash_invariant() {
    let src = r#"
import <string>;
import <hash>;

fn main() -> i32 {
    dec s1 = string_from_str("banana");
    dec rw s2 = string_with_capacity(32 as u64);
    s2.push_str("banana");

    if s1.hash_string() != s2.hash_string() {
        return 1;
    }

    dec s3 = string_new();
    dec s4 = string_with_capacity(16 as u64);
    if s3.hash_string() != s4.hash_string() {
        return 2;
    }

    dec s5 = string_from_str("apple");
    if s1.hash_string() == s5.hash_string() {
        return 3;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_string_hash", src);
    assert_eq!(code, 0, "String hash test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-04: Test 3
/// Verifies String as HashMap<String, i32> key.
#[test]
fn test_hashmap_with_string_keys() {
    let src = r#"
import <string>;
import <hashmap>;

fn main() -> i32 {
    dec rw map: HashMap<String, i32> = hashmap_new<String, i32>();

    dec k1 = string_from_str("foo");
    dec k2 = string_from_str("bar");
    dec k3 = string_from_str("baz");

    map.insert(k1, 100);
    map.insert(k2, 200);
    map.insert(k3, 300);

    if map.len() != (3 as u64) {
        return 1;
    }

    // Lookup using newly constructed keys with separate memory
    dec query_foo = string_from_str("foo");
    dec v_foo = map.get(&query_foo);
    match v_foo {
        Option::Some(val) -> {
            if *val != 100 {
                return 2;
            }
        },
        Option::None -> {
            return 3;
        },
    }

    dec query_bar = string_from_str("bar");
    dec v_bar = map.get(&query_bar);
    match v_bar {
        Option::Some(val) -> {
            if *val != 200 {
                return 4;
            }
        },
        Option::None -> {
            return 5;
        },
    }

    // Overwrite key
    dec k1_again = string_from_str("foo");
    map.insert(k1_again, 999);
    dec v_foo2 = map.get(&query_foo);
    match v_foo2 {
        Option::Some(val) -> {
            if *val != 999 {
                return 6;
            }
        },
        Option::None -> {
            return 7;
        },
    }

    // Remove key
    dec query_baz = string_from_str("baz");
    dec removed = map.remove(&query_baz);
    if removed == false {
        return 8;
    }
    if map.contains_key(&query_baz) {
        return 9;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_hashmap_string", src);
    assert_eq!(code, 0, "HashMap<String, i32> test failed with code {}. Stderr: {}", code, stderr);
}

/// STDLIB-GAP-04: Test 4
/// Verifies String as HashSet<String> element.
#[test]
fn test_hashset_with_string_elements() {
    let src = r#"
import <string>;
import <hashset>;

fn main() -> i32 {
    dec rw set: HashSet<String> = hashset_new<String>();

    dec s1 = string_from_str("alpha");
    dec s2 = string_from_str("beta");
    dec s3 = string_from_str("gamma");

    set.insert(s1);
    set.insert(s2);
    set.insert(s3);

    if set.len() != (3 as u64) {
        return 1;
    }

    dec query_alpha = string_from_str("alpha");
    if set.contains(&query_alpha) == false {
        return 2;
    }

    dec query_unknown = string_from_str("delta");
    if set.contains(&query_unknown) {
        return 3;
    }

    dec query_beta = string_from_str("beta");
    dec removed = set.remove(&query_beta);
    if removed == false {
        return 4;
    }
    if set.contains(&query_beta) {
        return 5;
    }

    return 0;
}
"#;
    let (code, _, stderr) = compile_and_run("test_hashset_string", src);
    assert_eq!(code, 0, "HashSet<String> test failed with code {}. Stderr: {}", code, stderr);
}
