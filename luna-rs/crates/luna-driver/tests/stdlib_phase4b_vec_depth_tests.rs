use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_p4b_vec_{}_{}_{}",
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

fn run_binary(dir: &Path, src: &str, opts: &CompilerOptions) -> Result<(i32, String, String), String> {
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

#[test]
fn test_vec_swap_remove() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("swap_remove");

    let src = r#"
        import <vec>;

        fn get_val(v: &Vec<i32>, idx: u64) -> i32 {
            match v.get(idx) {
                Option::Some(x) -> *x,
                Option::None -> 0 - 1,
            }
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(20);
            v.push(30);
            v.push(40);

            // swap_remove(1) removes 20, moves 40 into index 1
            dec removed = v.swap_remove(1 as u64);
            if removed != 20 { return 1; }
            if v.len() != (3 as u64) { return 2; }
            if get_val(&v, 0 as u64) != 10 { return 3; }
            if get_val(&v, 1 as u64) != 40 { return 4; }
            if get_val(&v, 2 as u64) != 30 { return 5; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_vec_swap_remove failed with code {}: {}", code, stderr);
}

#[test]
fn test_vec_retain() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("retain");

    let src = r#"
        import <vec>;

        fn get_val(v: &Vec<i32>, idx: u64) -> i32 {
            match v.get(idx) {
                Option::Some(x) -> *x,
                Option::None -> 0 - 1,
            }
        }

        fn is_even(x: &i32) -> bool {
            return (*x % 2) == 0;
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(1);
            v.push(2);
            v.push(3);
            v.push(4);
            v.push(5);
            v.push(6);

            v.retain(is_even);
            if v.len() != (3 as u64) { return 1; }
            if get_val(&v, 0 as u64) != 2 { return 2; }
            if get_val(&v, 1 as u64) != 4 { return 3; }
            if get_val(&v, 2 as u64) != 6 { return 4; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_vec_retain failed with code {}: {}", code, stderr);
}

#[test]
fn test_vec_dedup() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("dedup");

    let src = r#"
        import <vec>;

        fn get_val(v: &Vec<i32>, idx: u64) -> i32 {
            match v.get(idx) {
                Option::Some(x) -> *x,
                Option::None -> 0 - 1,
            }
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(1);
            v.push(1);
            v.push(2);
            v.push(3);
            v.push(3);
            v.push(3);
            v.push(4);

            v.dedup_i32();
            if v.len() != (4 as u64) { return 1; }
            if get_val(&v, 0 as u64) != 1 { return 2; }
            if get_val(&v, 1 as u64) != 2 { return 3; }
            if get_val(&v, 2 as u64) != 3 { return 4; }
            if get_val(&v, 3 as u64) != 4 { return 5; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_vec_dedup failed with code {}: {}", code, stderr);
}

#[test]
fn test_vec_insert_and_remove() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("insert_remove");

    let src = r#"
        import <vec>;

        fn get_val(v: &Vec<i32>, idx: u64) -> i32 {
            match v.get(idx) {
                Option::Some(x) -> *x,
                Option::None -> 0 - 1,
            }
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(10);
            v.push(30);

            // insert 20 at index 1 -> [10, 20, 30]
            v.insert(1 as u64, 20);
            if v.len() != (3 as u64) { return 1; }
            if get_val(&v, 1 as u64) != 20 { return 2; }

            // remove at index 1 -> removes 20 -> [10, 30]
            dec rem = v.remove(1 as u64);
            if rem != 20 { return 3; }
            if v.len() != (2 as u64) { return 4; }
            if get_val(&v, 0 as u64) != 10 { return 5; }
            if get_val(&v, 1 as u64) != 30 { return 6; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_vec_insert_and_remove failed with code {}: {}", code, stderr);
}

#[test]
fn test_vec_resize_and_extend() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("resize_extend");

    let src = r#"
        import <vec>;
        import <slice>;

        fn get_val(v: &Vec<i32>, idx: u64) -> i32 {
            match v.get(idx) {
                Option::Some(x) -> *x,
                Option::None -> 0 - 1,
            }
        }

        fn default_val() -> i32 {
            return 99;
        }

        fn main() -> i32 {
            dec rw v = vec_new<i32>();
            v.push(1);
            v.push(2);

            // resize_with up to 4 elements
            v.resize_with(4 as u64, default_val);
            if v.len() != (4 as u64) { return 1; }
            if get_val(&v, 2 as u64) != 99 { return 2; }
            if get_val(&v, 3 as u64) != 99 { return 3; }

            // resize_with down to 1 element
            v.resize_with(1 as u64, default_val);
            if v.len() != (1 as u64) { return 4; }
            if get_val(&v, 0 as u64) != 1 { return 5; }

            // extend_from_iter with owned elements from another Vec
            dec rw v2 = vec_new<i32>();
            v2.push(10);
            v2.push(20);
            v2.push(30);
            v.extend_from_iter(v2.into_iter());
            if v.len() != (4 as u64) { return 6; }
            if get_val(&v, 1 as u64) != 10 { return 7; }
            if get_val(&v, 2 as u64) != 20 { return 8; }
            if get_val(&v, 3 as u64) != 30 { return 9; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];

    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("Execution must succeed");
    assert_eq!(code, 0, "test_vec_resize_and_extend failed with code {}: {}", code, stderr);
}
