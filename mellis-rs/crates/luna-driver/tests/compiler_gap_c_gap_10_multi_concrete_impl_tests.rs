use std::{fs, path::PathBuf, process::Command};
use luna_driver::{compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;

fn create_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luna_c_gap_10_{}", name));
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

/// C10-1: Distinct concrete instantiations of parameterized trait on parameterized struct
/// Container<i32> with Action<i32> and Container<i64> with Action<i64>
#[test]
fn test_c10_1_distinct_concrete_instantiations() {
    let src = r#"
trait Action<T> {
    fn act(self: &Self, val: T) -> T;
}

struct Container<T> {
    val: T,
};

impl Action<i32> for Container<i32> {
    fn act(self: &Self, val: i32) -> i32 {
        return self.val + val;
    }
}

impl Action<i64> for Container<i64> {
    fn act(self: &Self, val: i64) -> i64 {
        return self.val + val;
    }
}

fn main() -> i32 {
    dec c1 = Container<i32> { val: 10 };
    dec c2 = Container<i64> { val: 20 as i64 };
    dec r1 = c1.act(32);
    dec r2 = c2.act(22 as i64);
    if r1 == 42 && r2 == (42 as i64) {
        return 0;
    }
    return 1;
}
"#;

    let (code, _, _) = compile_and_run("test_c10_1", src);
    assert_eq!(code, 0, "C10-1 must execute cleanly with exit code 0");
}

/// C10-2: Same self type implementing distinct instantiations of the same trait
/// Host implementing Consumer<ItemA> and Consumer<ItemB>
#[test]
fn test_c10_2_same_self_distinct_trait_args() {
    let src = r#"
trait Consumer<T> {
    fn consume(self: &Self, item: &T) -> i32;
}

struct ItemA {
    score: i32,
};

struct ItemB {
    weight: i32,
};

struct Host {
    id: i32,
};

impl Consumer<ItemA> for Host {
    fn consume(self: &Self, item: &ItemA) -> i32 {
        return self.id + item.score;
    }
}

impl Consumer<ItemB> for Host {
    fn consume(self: &Self, item: &ItemB) -> i32 {
        return self.id + item.weight * 2;
    }
}

fn main() -> i32 {
    dec host = Host { id: 10 };
    dec a = ItemA { score: 5 };
    dec b = ItemB { weight: 8 };
    dec r1 = host.consume(&a);
    dec r2 = host.consume(&b);
    if r1 == 15 && r2 == 26 {
        return 0;
    }
    return 1;
}
"#;

    let (code, _, _) = compile_and_run("test_c10_2", src);
    assert_eq!(code, 0, "C10-2 must execute cleanly with exit code 0");
}

/// C10-3: Generic impl instantiated at multiple concrete types
/// impl<T> Action<T> for Container<T> with i32 and i64
#[test]
fn test_c10_3_generic_impl_instantiations() {
    let src = r#"
trait Action<T> {
    fn act(self: &Self, val: T) -> T;
}

struct Container<T> {
    val: T,
};

impl<T> Action<T> for Container<T> {
    fn act(self: &Self, val: T) -> T {
        return val;
    }
}

fn main() -> i32 {
    dec c1 = Container<i32> { val: 100 };
    dec c2 = Container<i64> { val: 200 as i64 };
    dec r1 = c1.act(42);
    dec r2 = c2.act(42 as i64);
    if r1 == 42 && r2 == (42 as i64) {
        return 0;
    }
    return 1;
}
"#;

    let (code, _, _) = compile_and_run("test_c10_3", src);
    assert_eq!(code, 0, "C10-3 must execute cleanly with exit code 0");
}

/// C10-4: Generic relation preservation
/// impl<T> PairAction<T> for Pair<T, T>
#[test]
fn test_c10_4_generic_relation_preservation() {
    let src = r#"
trait PairAction<T> {
    fn sum_pair(self: &Self) -> T;
}

struct Pair<A, B> {
    first: A,
    second: B,
};

impl<T> PairAction<T> for Pair<T, T> {
    fn sum_pair(self: &Self) -> T {
        return self.first + self.second;
    }
}

fn main() -> i32 {
    dec p1 = Pair<i32, i32> { first: 20, second: 22 };
    dec p2 = Pair<i64, i64> { first: 30 as i64, second: 12 as i64 };
    dec r1 = p1.sum_pair();
    dec r2 = p2.sum_pair();
    if r1 == 42 && r2 == (42 as i64) {
        return 0;
    }
    return 1;
}
"#;

    let (code, _, _) = compile_and_run("test_c10_4", src);
    assert_eq!(code, 0, "C10-4 must execute cleanly with exit code 0");
}

/// C10-5: Coherence rejects overlapping generic and concrete impls
#[test]
fn test_c10_5_coherence_generic_concrete_overlap_rejection() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_c10_5");
    let main_path = temp.join("main.ln");

    let src = r#"
trait Action<T> {
    fn act(self: &Self, val: T) -> T;
}

struct Container<T> {
    val: T,
};

impl<T> Action<T> for Container<T> {
    fn act(self: &Self, val: T) -> T {
        return val;
    }
}

impl Action<i32> for Container<i32> {
    fn act(self: &Self, val: i32) -> i32 {
        return self.val + val;
    }
}

fn main() -> i32 {
    return 0;
}
"#;

    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Overlapping generic and concrete impls must be rejected by coherence");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_CONFLICTING_TRAIT_IMPL")),
        "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}",
        errs
    );
}

/// C10-6: Coherence rejects duplicate concrete impls
#[test]
fn test_c10_6_coherence_duplicate_concrete_rejection() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_c10_6");
    let main_path = temp.join("main.ln");

    let src = r#"
trait Action<T> {
    fn act(self: &Self, val: T) -> T;
}

struct Container<T> {
    val: T,
};

impl Action<i32> for Container<i32> {
    fn act(self: &Self, val: i32) -> i32 {
        return self.val + val;
    }
}

impl Action<i32> for Container<i32> {
    fn act(self: &Self, val: i32) -> i32 {
        return val;
    }
}

fn main() -> i32 {
    return 0;
}
"#;

    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Duplicate concrete impls must be rejected by coherence");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_CONFLICTING_TRAIT_IMPL")),
        "Expected E_CONFLICTING_TRAIT_IMPL diagnostic, got: {:?}",
        errs
    );
}

/// C10-7: .llib artifact roundtrip with multi-concrete and generic impl parity
#[test]
fn test_c10_7_llib_roundtrip_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_c10_7");

    // 1. Build provider library provider_c10.llib with both multi-concrete and generic impls
    let prov_path = temp.join("provider_c10.ln");
    let prov_llib = temp.join("provider_c10.llib");
    let prov_src = r#"
export trait Action<T> {
    fn act(self: &Self, val: T) -> T;
}

export struct Container<T> {
    export val: T,
};

export struct Boxed<T> {
    export item: T,
};

impl Action<i32> for Container<i32> {
    fn act(self: &Self, val: i32) -> i32 {
        return self.val + val;
    }
}

impl Action<i64> for Container<i64> {
    fn act(self: &Self, val: i64) -> i64 {
        return self.val + val;
    }
}

impl<T> Action<T> for Boxed<T> {
    fn act(self: &Self, val: T) -> T {
        return val;
    }
}
"#;
    fs::write(&prov_path, prov_src).unwrap();

    let prov_opts = CompilerOptions {
        output_path: Some(prov_llib.to_str().unwrap().to_string()),
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };

    let prov_res = compile(prov_path.to_str().unwrap(), prov_src.to_string(), &prov_opts);
    assert!(prov_res.is_ok(), "Compiling provider_c10.ln -> provider_c10.llib failed: {:?}", prov_res.err());
    assert!(prov_llib.is_file(), "provider_c10.llib must exist");

    // CRITICAL PROOF: Remove source file so consumer CANNOT access provider source
    fs::remove_file(&prov_path).unwrap();
    assert!(!prov_path.exists(), "Source provider file must not exist for .llib-only consumption");

    // 2. Compile consumer main.ln importing provider_c10 solely from .llib in a fresh session
    let main_path = temp.join("main.ln");
    let main_exe = temp.join("main.exe");
    let main_src = r#"
import "provider_c10";

fn main() -> i32 {
    dec c1 = Container<i32> { val: 10 };
    dec c2 = Container<i64> { val: 20 as i64 };
    dec b = Boxed<i32> { item: 99 };
    dec r1 = c1.act(32);
    dec r2 = c2.act(22 as i64);
    dec r3 = b.act(42);
    if r1 == 42 && r2 == (42 as i64) && r3 == 42 {
        return 0;
    }
    return 1;
}
"#;
    fs::write(&main_path, main_src).unwrap();

    let main_opts = CompilerOptions {
        output_path: Some(main_exe.to_str().unwrap().to_string()),
        search_paths: vec![
            temp.to_string_lossy().to_string(),
            test_sysroot.root().to_string_lossy().to_string(),
        ],
        emit_llib: false,
        no_link: false,
        quiet: false,
        ..Default::default()
    };

    let main_res = compile(main_path.to_str().unwrap(), main_src.to_string(), &main_opts);
    assert!(main_res.is_ok(), "Compiling consumer main.ln failed: {:?}", main_res.err());
    assert!(main_exe.is_file(), "Executable was not produced at {:?}", main_exe);

    let output = Command::new(&main_exe)
        .output()
        .expect("Failed to execute produced binary");

    let exit_code = output.status.code().unwrap_or(-1);
    assert_eq!(exit_code, 0, "C10-7 consumer importing provider_c10 solely from .llib must exit with code 0");
}

/// C10-8: Alpha-equivalent generic binders conflict under coherence
#[test]
fn test_c10_8_alpha_equivalent_generic_binders_conflict() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let temp = create_temp_dir("test_c10_8");
    let main_path = temp.join("main.ln");

    let src = r#"
trait Converter<T> {
    fn convert(self: &Self, val: T) -> T;
}

struct Wrapper<T> {
    val: T,
};

impl<T> Converter<T> for Wrapper<T> {
    fn convert(self: &Self, val: T) -> T {
        return val;
    }
}

impl<U> Converter<U> for Wrapper<U> {
    fn convert(self: &Self, val: U) -> U {
        return val;
    }
}

fn main() -> i32 {
    return 0;
}
"#;

    fs::write(&main_path, src).unwrap();

    let options = CompilerOptions {
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        quiet: true,
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), src.to_string(), &options);
    assert!(compile_res.is_err(), "Alpha-equivalent generic impls (T vs U) must be rejected by coherence");
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("E_CONFLICTING_TRAIT_IMPL")),
        "Expected E_CONFLICTING_TRAIT_IMPL diagnostic for alpha-equivalent impl heads, got: {:?}",
        errs
    );
}

