use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "luna_test_p4b_cmp_{}_{}_{}",
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
fn test_cmp_eq_primitives() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("cmp_eq");

    let src = r#"
        import <cmp>;

        fn main() -> i32 {
            dec a: i32 = 42;
            dec b: i32 = 42;
            dec c: i32 = 99;

            if a.eq(&b) == false { return 1; }
            if a.eq(&c) == true { return 2; }

            dec u1: u64 = 1000 as u64;
            dec u2: u64 = 1000 as u64;
            dec u3: u64 = 2000 as u64;
            if u1.eq(&u2) == false { return 3; }
            if u1.eq(&u3) == true { return 4; }

            dec b1 = true;
            dec b2 = false;
            if b1.eq(&b1) == false { return 5; }
            if b1.eq(&b2) == true { return 6; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_cmp_ord_primitives() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("cmp_ord");

    let src = r#"
        import <cmp>;

        fn main() -> i32 {
            dec a: i32 = 10;
            dec b: i32 = 20;
            dec c: i32 = 10;

            if a.cmp(&b) != (0 - 1) { return 1; }
            if b.cmp(&a) != 1 { return 2; }
            if a.cmp(&c) != 0 { return 3; }

            dec u_lo: u64 = 5 as u64;
            dec u_hi: u64 = 500 as u64;
            if u_lo.cmp(&u_hi) != (0 - 1) { return 4; }
            if u_hi.cmp(&u_lo) != 1 { return 5; }
            if u_lo.cmp(&u_lo) != 0 { return 6; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_cmp_min_max() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("cmp_min_max");

    let src = r#"
        import <cmp>;

        fn main() -> i32 {
            if min_i32(10, 20) != 10 { return 1; }
            if min_i32(20, 10) != 10 { return 2; }
            if max_i32(10, 20) != 20 { return 3; }
            if max_i32(20, 10) != 20 { return 4; }

            dec u10 = 10 as u64;
            dec u20 = 20 as u64;
            if min_u64(u10, u20) != u10 { return 5; }
            if max_u64(u10, u20) != u20 { return 6; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn test_cmp_clamp() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("cmp_clamp");

    let src = r#"
        import <cmp>;

        fn main() -> i32 {
            // within range
            if clamp_i32(15, 10, 20) != 15 { return 1; }
            // below range
            if clamp_i32(5, 10, 20) != 10 { return 2; }
            // above range
            if clamp_i32(25, 10, 20) != 20 { return 3; }

            dec lo = 10 as u64;
            dec hi = 20 as u64;
            if clamp_u64(15 as u64, lo, hi) != (15 as u64) { return 4; }
            if clamp_u64(5 as u64, lo, hi) != lo { return 5; }
            if clamp_u64(25 as u64, lo, hi) != hi { return 6; }

            return 0;
        }
    "#;

    let mut opts = CompilerOptions::default();
    opts.search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    let (code, _stdout, stderr) = run_binary(&dir, src, &opts).expect("execution should succeed");
    assert_eq!(code, 0, "stderr: {}", stderr);
}
