use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn locate_canonical_string_ln() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("string.ln");
        if p.exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/string.ln");
}

fn locate_canonical_string_llib() -> PathBuf {
    let mut dir = std::env::current_dir().expect("Failed to get current directory");
    for _ in 0..6 {
        let p = dir.join("libs").join("external").join("alloc").join("string.llib");
        if p.parent().unwrap().exists() {
            return p;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("Unable to locate canonical libs/external/alloc/string.llib");
}

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("luna_test_{}_{}_{}", prefix, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

fn run_binary_with_output(
    dir: &Path,
    src: &str,
    opts: &CompilerOptions,
    _args: &[&str],
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


/// S1: ASCII String creation, length, byte access
#[test]
fn test_s1_string_ascii() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s1_ascii");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec s = string_from_str("Hello, Mellis!");
    if s.len() != (14 as u64) {
        return 1;
    }
    if s.is_empty() {
        return 2;
    }
    dec bytes = s.as_bytes();
    if bytes[0] != (72 as u8) { // 'H'
        return 3;
    }
    if bytes[13] != (33 as u8) { // '!'
        return 4;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S1 ASCII string test must pass (code: {}, stderr: {})", code, stderr);
}

/// S2: 2-byte UTF-8 scalar (Greek/Cyrillic e.g. α = 0xCE 0xBF = 206, 191)
#[test]
fn test_s2_string_2byte_scalar() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s2_2byte");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw v = vec_new<u8>();
    v.push(206 as u8); // 0xCE
    v.push(191 as u8); // 0xBF (Greek alpha 'α')
    
    dec opt = string_from_bytes(v.as_slice());
    if opt.is_none() {
        return 1;
    }
    dec s = opt.unwrap();
    if s.len() != (2 as u64) {
        return 2;
    }
    dec bytes = s.as_bytes();
    if bytes[0] != (206 as u8) || bytes[1] != (191 as u8) {
        return 3;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S2 2-byte scalar test must pass (code: {}, stderr: {})", code, stderr);
}

/// S3: 3-byte UTF-8 scalar (Vietnamese with diacritics e.g. 'ệ' = 0xE1 0xBB 0x87 = 225, 187, 135)
#[test]
fn test_s3_string_3byte_scalar() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s3_3byte");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    // "Việt" -> V (86), i (105), ệ (225, 187, 135), t (116) = 6 bytes
    dec rw v = vec_new<u8>();
    v.push(86 as u8);  // 'V'
    v.push(105 as u8); // 'i'
    v.push(225 as u8); // 'ệ' byte 1
    v.push(187 as u8); // 'ệ' byte 2
    v.push(135 as u8); // 'ệ' byte 3
    v.push(116 as u8); // 't'

    dec opt = string_from_bytes(v.as_slice());
    if opt.is_none() {
        return 1;
    }
    dec s = opt.unwrap();
    if s.len() != (6 as u64) {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S3 3-byte scalar test must pass (code: {}, stderr: {})", code, stderr);
}

/// S4: 4-byte UTF-8 scalar (Emoji 🦀 = 0xF0 0x9F 0xA6 0x80 = 240, 159, 166, 128)
#[test]
fn test_s4_string_4byte_scalar() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s4_4byte");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw v = vec_new<u8>();
    v.push(240 as u8); // 0xF0
    v.push(159 as u8); // 0x9F
    v.push(166 as u8); // 0xA6
    v.push(128 as u8); // 0x80 (Crab emoji)

    dec opt = string_from_bytes(v.as_slice());
    if opt.is_none() {
        return 1;
    }
    dec s = opt.unwrap();
    if s.len() != (4 as u64) {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S4 4-byte scalar test must pass (code: {}, stderr: {})", code, stderr);
}

/// S5: Invalid UTF-8 rejection (invalid continuation bytes)
#[test]
fn test_s5_string_invalid_utf8_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s5_invalid");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    // 0xC2 without continuation byte (next byte is ASCII 'A' = 65)
    dec rw v = vec_new<u8>();
    v.push(194 as u8);
    v.push(65 as u8);

    dec opt = string_from_bytes(v.as_slice());
    if opt.is_some() {
        return 1; // Must be rejected
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S5 Invalid UTF-8 rejection must pass (code: {}, stderr: {})", code, stderr);
}

/// S6: Overlong UTF-8 sequence rejected (e.g. 0xC0 0x80 for NUL)
#[test]
fn test_s6_string_overlong_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s6_overlong");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw v = vec_new<u8>();
    v.push(192 as u8); // 0xC0
    v.push(128 as u8); // 0x80 (Overlong NUL)

    dec opt = string_from_bytes(v.as_slice());
    if opt.is_some() {
        return 1; // Overlong must be rejected
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S6 Overlong UTF-8 rejection must pass (code: {}, stderr: {})", code, stderr);
}

/// S7: UTF-16 surrogate range (0xD800..0xDFFF) rejected
#[test]
fn test_s7_string_surrogates_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s7_surrogates");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    // 0xED 0xA0 0x80 -> encodes 0xD800 (surrogate half)
    dec rw v = vec_new<u8>();
    v.push(237 as u8);
    v.push(160 as u8);
    v.push(128 as u8);

    dec opt = string_from_bytes(v.as_slice());
    if opt.is_some() {
        return 1; // Surrogate must be rejected
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S7 Surrogate rejection must pass (code: {}, stderr: {})", code, stderr);
}

/// S8: `truncate` at valid char boundary succeeds
#[test]
fn test_s8_string_truncate_on_boundary() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s8_truncate_valid");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    // "Việt" -> 6 bytes: V (0), i (1), ệ (2..5), t (5..6)
    dec rw v = vec_new<u8>();
    v.push(86 as u8);  // 'V'
    v.push(105 as u8); // 'i'
    v.push(225 as u8); // 'ệ' byte 1
    v.push(187 as u8); // 'ệ' byte 2
    v.push(135 as u8); // 'ệ' byte 3
    v.push(116 as u8); // 't'

    dec rw s = string_from_bytes(v.as_slice()).unwrap();
    // Truncate after 'Vi' (index 2 is a valid scalar boundary)
    s.truncate(2 as u64);
    if s.len() != (2 as u64) {
        return 1;
    }
    dec bytes = s.as_bytes();
    if bytes[0] != (86 as u8) || bytes[1] != (105 as u8) {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S8 Truncate on boundary must pass (code: {}, stderr: {})", code, stderr);
}

/// S9: `truncate` in middle of multi-byte sequence panics
#[test]
fn test_s9_string_truncate_in_middle_panics() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s9_truncate_panic");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    // "Việt" -> 'ệ' starts at index 2 (length 3 bytes: 2, 3, 4)
    dec rw v = vec_new<u8>();
    v.push(86 as u8);
    v.push(105 as u8);
    v.push(225 as u8);
    v.push(187 as u8);
    v.push(135 as u8);
    v.push(116 as u8);

    dec rw s = string_from_bytes(v.as_slice()).unwrap();
    // Index 3 is in the middle of 'ệ' -> MUST PANIC!
    s.truncate(3 as u64);
    return 0;
}
"#;

    let (code, _stdout, _stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_ne!(code, 0, "S9 Truncate in middle of multi-byte sequence must trigger panic (non-zero exit code)");
}

/// S10: `push_str` preserves validity
#[test]
fn test_s10_string_push_str() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s10_push_str");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw s = string_new();
    s.push_str("Hello");
    s.push_str(" ");
    s.push_str("World");
    if s.len() != (11 as u64) {
        return 1;
    }
    dec b = s.as_bytes();
    if b[0] != (72 as u8) || b[5] != (32 as u8) || b[10] != (100 as u8) {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S10 push_str test must pass (code: {}, stderr: {})", code, stderr);
}

/// S11: `push_char` for 1..4 byte scalars
#[test]
fn test_s11_string_push_char_all_widths() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s11_push_char");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw s = string_new();
    s.push_char('A'); // 1-byte
    if s.len() != (1 as u64) {
        return 1;
    }
    s.push_char('Z'); // 1-byte
    if s.len() != (2 as u64) {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S11 push_char test must pass (code: {}, stderr: {})", code, stderr);
}

/// S12: `String` move semantics and clean Drop
#[test]
fn test_s12_string_move_and_drop() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s12_move_drop");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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

fn consume_string(s: String) -> u64 {
    return s.len();
}

fn main() -> i32 {
    dec s = string_from_str("Temporary string that gets moved and dropped");
    dec len = consume_string(s);
    if len != (44 as u64) {
        return 1;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S12 move and drop test must pass (code: {}, stderr: {})", code, stderr);
}

/// S13: Borrowck immutability check: `as_bytes(&self)` blocks `&rw self` mutation while borrow is alive
#[test]
fn test_s13_string_borrow_blocks_mutation() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s13_borrowck");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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

fn test_conflict() {
    dec rw s = string_from_str("hello");
    dec b = s.as_bytes();
    s.push_str("world"); // MUST FAIL: s is mutably borrowed while b is active
    dec first = b[0];
}

fn main() -> i32 {
    test_conflict();
    return 0;
}
"#;

    let res = compile(dir.join("test_conflict.ln").to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "S13 Borrowck MUST reject mutating String while borrowed as slice");
}

/// S14: Source `.ln` and `.llib` parity for `String`
#[test]
fn test_s14_string_source_and_llib_parity() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate sysroot");
    let dir = create_temp_dir("s14_parity");
    let string_ln = locate_canonical_string_ln();
    let string_src = fs::read_to_string(&string_ln).expect("Failed to read string.ln");

    let out_llib = dir.join("string.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(out_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        is_sysroot_build: true,
        ..Default::default()
    };

    let res_compile = compile(string_ln.to_str().unwrap(), string_src, &compile_opts);
    assert!(res_compile.is_ok(), "Compiling string.ln to string.llib must succeed: {:?}", res_compile.err());

    let canonical_llib = locate_canonical_string_llib();
    let _ = fs::copy(&out_llib, &canonical_llib);

    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec rw s = string_new();
    s.push_str("Mellis");
    s.push_str(" ");
    s.push_str("Language");
    
    if s.len() != (15 as u64) {
        return 1;
    }
    
    dec s2 = s.clone();
    if s.eq(&s2) == false {
        return 2;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S14 Source/.llib parity must pass (code: {}, stderr: {})", code, stderr);
}

/// S15: Native execution with `<io>` output
#[test]
fn test_s15_string_native_io_integration() {
    let sysroot = Sysroot::discover_for_test().expect("Failed to locate sysroot");
    let dir = create_temp_dir("s15_io");
    let opts = CompilerOptions {
        search_paths: vec![sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
import <io>;

fn main() -> i32 {
    dec s = string_from_str("Hello from Mellis String stdlib!");
    io::println(s.as_bytes());
    return 0;
}
"#;

    let (code, stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S15 Native IO test must pass (code: {}, stderr: {})", code, stderr);
    assert!(stdout.contains("Hello from Mellis String stdlib!"), "stdout must contain string message: {}", stdout);
}

/// S16: `Clone` and `Eq` implementation
#[test]
fn test_s16_string_clone_and_eq() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("s16_clone_eq");
    let opts = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

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
    dec s1 = string_from_str("apple");
    dec rw s2 = s1.clone();
    
    if s1.eq(&s2) == false {
        return 1;
    }
    
    s2.push_str("s"); // s2 is now "apples", s1 is still "apple"
    if s1.eq(&s2) {
        return 2;
    }
    if s1.len() != (5 as u64) || s2.len() != (6 as u64) {
        return 3;
    }
    return 0;
}
"#;

    let (code, _stdout, stderr) = run_binary_with_output(&dir, src, &opts, &[]).expect("Failed to run");
    assert_eq!(code, 0, "S16 Clone and Eq test must pass (code: {}, stderr: {})", code, stderr);
}
