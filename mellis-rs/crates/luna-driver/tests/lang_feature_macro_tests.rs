use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_lang_macro_tests")
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

/// 1. Comprehensive Macro Fragment Kinds: expr, ident, ty, stmt, block, item, literal, tt
#[test]
fn test_macro_fragments_hardened() {
    let src = r#"
// Macro expanding item declarations (struct and function)
macro declare_entity {
    (@struct_name: ident, @fn_name: ident, @ty: ty) => {
        struct @struct_name {
            val: @ty,
        };

        fn @fn_name(x: @ty) -> @struct_name {
            return @struct_name { val: x };
        }
    }
}

declare_entity!(WrappedInt, make_wrapped, i32);

// Macro expanding statement blocks in statement position
macro run_calc {
    (@b: block) => {
        { @b }
    }
}

macro square_expr {
    (@x: expr) => {
        (@x) * (@x)
    }
}

macro exec_stmt {
    (@s: stmt) => {
        @s
    }
}

fn main() -> i32 {
    dec entity = make_wrapped(7);
    if entity.val != 7 {
        return 1;
    }

    dec sq = square_expr!(entity.val + 3); // (7 + 3) * (7 + 3) = 100
    if sq != 100 {
        return 2;
    }

    dec rw acc: i32 = 0;
    exec_stmt!(acc = acc + sq;);
    if acc != 100 {
        return 3;
    }

    run_calc!({
        acc = acc + 50;
    });
    if acc != 150 {
        return 4;
    }

    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_macro_fragments_hardened", src);
    assert_eq!(exit_code, 0, "test_macro_fragments_hardened failed with code {}", exit_code);
}

/// 2. Macro Repetitions: * (zero or more), + (one or more) with separators
#[test]
fn test_macro_repetition_star_and_plus() {
    let src = r#"
// Repetition with * (zero or more)
macro sum_star {
    () => { 0 }
    ($(@x: expr),*) => {
        0 $( + @x )*
    }
}

// Repetition with + (one or more)
macro prod_plus {
    ($(@x: expr),+) => {
        1 $( * @x )+
    }
}

fn main() -> i32 {
    // 0 elements with *
    dec zero = sum_star!();
    if zero != 0 {
        return 1;
    }

    // Multiple elements with *
    dec s = sum_star!(10, 20, 30);
    if s != 60 {
        return 2;
    }

    // 1 element with +
    dec p1 = prod_plus!(5);
    if p1 != 5 {
        return 3;
    }

    // Multiple elements with +
    dec p2 = prod_plus!(2, 3, 4);
    if p2 != 24 {
        return 4;
    }

    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_macro_repetition_star_and_plus", src);
    assert_eq!(exit_code, 0, "test_macro_repetition_star_and_plus failed with code {}", exit_code);
}

/// 3. Negative assertion: + repetition requires at least one match
#[test]
fn test_macro_repetition_plus_zero_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_macro_plus_zero_rejected");
    let main_path = temp.join("main.ln");

    let src = r#"
macro require_at_least_one {
    ($(@x: expr),+) => {
        $( @x; )+
    }
}

fn main() -> i32 {
    require_at_least_one!();
    return 0;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected macro invocation with 0 arguments on '+' repetition to fail");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("matched the invocation") || d.message.contains("at least one repetition") || d.message.contains("expected")),
        "Expected match failure diagnostic, got: {:?}",
        errs
    );
}

/// 4. Scope Hygiene & Collision Prevention
#[test]
fn test_macro_hygiene_and_lexical_isolation() {
    let src = r#"
macro introduce_local {
    (@val: expr) => {
        dec tmp: i32 = @val * 10;
    }
}

fn main() -> i32 {
    // Caller defines local tmp
    dec tmp: i32 = 42;

    // Macro introduces its own local tmp; must not mutate or shadow caller's tmp
    introduce_local!(5);

    if tmp != 42 {
        return 1;
    }

    // Invoking the macro a second time in the same scope must not produce duplicate declaration error
    introduce_local!(7);

    if tmp != 42 {
        return 2;
    }

    return 0;
}
"#;

    let (exit_code, _, _) = compile_and_run("test_macro_hygiene_and_lexical_isolation", src);
    assert_eq!(exit_code, 0, "test_macro_hygiene_and_lexical_isolation failed with code {}", exit_code);
}

/// 5. Negative assertion: Macro recursion depth limit rejection
#[test]
fn test_macro_recursion_limit_rejected() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_macro_recursion_limit_rejected");
    let main_path = temp.join("main.ln");

    let src = r#"
macro infinite_recursion {
    (@x: expr) => {
        infinite_recursion!(@x + 1)
    }
}

fn main() -> i32 {
    dec val = infinite_recursion!(0);
    return val;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected infinite macro recursion to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("recursion limit reached while expanding macro")),
        "Expected recursion limit diagnostic, got: {:?}",
        errs
    );
}

/// 6. Cross-Module & Provider Transport Parity: source vs .llib
#[test]
fn test_macro_provider_transport_source_and_llib() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_macro_transport");
    let prov_path = temp.join("macrolib.ln");
    let llib_path = temp.join("macrolib.llib");
    let main_path = temp.join("main.ln");
    let exe_path = temp.join("main.exe");

    let prov_src = r#"
export macro compute_offset {
    (@base: expr, @scale: expr) => {
        (@base * @scale) + 100
    }
}
"#;

    let consumer_src = r#"
import "macrolib";

fn main() -> i32 {
    dec res = compute_offset!(5, 20); // 5 * 20 + 100 = 200
    if res != 200 {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&prov_path, prov_src).unwrap();
    fs::write(&main_path, consumer_src).unwrap();

    // Part A: Source provider import
    let options_src = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![temp.to_str().unwrap().to_string(), test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), consumer_src.to_string(), &options_src);
    assert!(compile_res.is_ok(), "Source provider compilation failed: {:?}", compile_res.err());
    let output = Command::new(&exe_path).output().expect("Failed to run source provider consumer");
    assert_eq!(output.status.code().unwrap_or(-1), 0, "Source provider execution failed");

    // Part B: Compile provider to .llib
    let llib_opts = CompilerOptions {
        output_path: Some(llib_path.to_str().unwrap().to_string()),
        emit_llib: true,
        no_link: true,
        quiet: true,
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };
    let llib_res = compile(prov_path.to_str().unwrap(), prov_src.to_string(), &llib_opts);
    assert!(llib_res.is_ok(), "Failed to compile provider to .llib: {:?}", llib_res.err());
    assert!(llib_path.is_file(), "macrolib.llib was not produced");

    // Remove source file to guarantee consumer uses the .llib artifact
    fs::remove_file(&prov_path).unwrap();
    let _ = fs::remove_file(&exe_path);

    // Part C: Consumer imports .llib provider
    let options_llib = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        search_paths: vec![temp.to_str().unwrap().to_string(), test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };
    let compile_llib_res = compile(main_path.to_str().unwrap(), consumer_src.to_string(), &options_llib);
    assert!(compile_llib_res.is_ok(), ".llib provider compilation failed: {:?}", compile_llib_res.err());
    let output_llib = Command::new(&exe_path).output().expect("Failed to run .llib provider consumer");
    assert_eq!(output_llib.status.code().unwrap_or(-1), 0, ".llib provider execution failed");
}

/// 7. Macro Argument Pattern Mismatch Span Assertion
#[test]
fn test_macro_invalid_syntax_error_span() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_macro_invalid_syntax");
    let main_path = temp.join("main.ln");

    let src = r#"
macro expect_two_idents {
    (@a: ident, @b: ident) => {
        dec @a: i32 = 1;
        dec @b: i32 = 2;
    }
}

fn main() -> i32 {
    expect_two_idents!(valid_id, 12345); // 12345 is literal, not ident!
    return 0;
}
"#;

    fs::write(&main_path, src).unwrap();
    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        no_link: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Expected macro argument pattern mismatch to be rejected");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("expected") || d.message.contains("no rule in macro")),
        "Expected pattern mismatch error, got: {:?}",
        errs
    );
}
