//! Phase 4F: Artifact metadata parity and transitive artifact-chain identity.
//!
//! These are Layer 2 / Layer 3 tests. They exercise the real production
//! serialization/deserialization path (`compile` -> `.llib` -> discovery -> load)
//! and assert observable semantic parity, not internal allocation order.
//!
//! Bug classes proven here:
//!   1. Same-name generic parameters (`First<T>`, `Second<T>`) must stay
//!      declaration-scoped; they must not collapse to a single global `T`.
//!   2. A type owned by provider A, re-exposed through provider B's public
//!      interface, must still be reconstructed as A's type by consumer C
//!      (transitive nominal type identity).
//!   3. Transitively re-exposed generic instantiations must preserve owner,
//!      type constructor, argument order, and argument identity.
//!   4. Trait method resolution must survive a transitive A -> B -> C chain.
//!   5. Mixed source/artifact graphs must behave identically to all-artifact.

use luna_driver::sysroot::Sysroot;
use luna_driver::{compile, CompilerOptions};
use luna_llib::metadata::CanonicalType;
use luna_llib::MlibReader;
use luna_semantic::ty::Mutability;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "luna_meta_parity_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sysroot_root() -> String {
    Sysroot::discover_for_test()
        .expect("Failed to locate test sysroot")
        .root()
        .to_string_lossy()
        .to_string()
}

fn build_provider(dir: &Path, name: &str, src: &str, sysroot: &str) {
    let ln = dir.join(format!("{}.ln", name));
    let llib = dir.join(format!("{}.llib", name));
    fs::write(&ln, src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(llib.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: true,
        no_link: true,
        quiet: true,
        ..Default::default()
    };
    let res = compile(ln.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "build provider {} failed: {:?}", name, res.err());
    assert!(llib.exists(), "provider {}.llib was not produced", name);
}

fn run_consumer(dir: &Path, main_src: &str, sysroot: &str) -> (bool, i32) {
    let main_path = dir.join("main.ln");
    let exe = dir.join("main.exe");
    let _ = fs::remove_file(&exe);
    fs::write(&main_path, main_src).unwrap();
    let opts = CompilerOptions {
        output_path: Some(exe.to_string_lossy().to_string()),
        search_paths: vec![dir.to_string_lossy().to_string(), sysroot.to_string()],
        emit_llib: false,
        no_link: false,
        quiet: true,
        ..Default::default()
    };
    let res = compile(main_path.to_str().unwrap(), main_src.to_string(), &opts);
    if let Err(e) = res {
        eprintln!("consumer compile failed: {:?}", e);
        return (false, -1);
    }
    let output = Command::new(&exe).output().expect("run consumer");
    (true, output.status.code().unwrap_or(-2))
}

// ---------------------------------------------------------------------------
// 1. Same-name generic parameters must stay declaration-scoped.
// ---------------------------------------------------------------------------
#[test]
fn test_generic_same_name_params_declaration_scoped() {
    let dir = temp_dir("generic_same_name");
    let sysroot = sysroot_root();

    let provider = r#"
module gen {
    export struct First<T> {
        export value: T,
    }

    export struct Second<T> {
        export value: T,
    }

    impl<T> First<T> {
        export fn get(self: &First<T>) -> T {
            return self.value;
        }
    }

    impl<T> Second<T> {
        export fn get(self: &Second<T>) -> T {
            return self.value;
        }
    }
}
"#;
    build_provider(&dir, "gen", provider, &sysroot);

    let main_src = r#"
import "gen";

fn main() -> i32 {
    dec a = gen::First<i32> { value: 40 };
    dec b = gen::Second<i32> { value: 2 };
    dec va = a.get();
    dec vb = b.get();
    if va + vb == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, main_src, &sysroot);
    assert!(ok, "consumer must compile against .llib");
    assert_eq!(code, 0, "same-name generic params must stay declaration-scoped");
}

// ---------------------------------------------------------------------------
// 2. Transitive nominal type identity: A -> B -> C.
// ---------------------------------------------------------------------------
#[test]
fn test_transitive_nominal_type_identity_a_b_c() {
    let dir = temp_dir("transitive_nominal");
    let sysroot = sysroot_root();

    let a = r#"
module alpha {
    export struct Item {
        export x: i32,
    }

    export fn make(x: i32) -> Item {
        return Item { x: x };
    }

    export fn read(it: &Item) -> i32 {
        return it.x;
    }
}
"#;
    build_provider(&dir, "a", a, &sysroot);

    let b = r#"
import "a";

module beta {
    export fn produce(v: i32) -> alpha::Item {
        return alpha::make(v);
    }

    export fn consume(it: alpha::Item) -> i32 {
        return alpha::read(&it);
    }
}
"#;
    build_provider(&dir, "b", b, &sysroot);

    // C imports ONLY B; the A-owned identity must flow through B's public interface.
    let c = r#"
import "b";

fn main() -> i32 {
    dec it = beta::produce(42);
    if beta::consume(it) == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "C must compile against B.llib");
    assert_eq!(code, 0, "A-owned nominal identity must survive A -> B -> C");
}

// ---------------------------------------------------------------------------
// 3. Transitive generic instantiation identity: A -> B -> C.
// ---------------------------------------------------------------------------
#[test]
fn test_transitive_generic_type_args_a_b_c() {
    let dir = temp_dir("transitive_generic");
    let sysroot = sysroot_root();

    let a = r#"
module alpha {
    export struct Wrapper<T> {
        export inner: T,
    }

    export fn wrap(x: i32) -> Wrapper<i32> {
        return Wrapper<i32> { inner: x };
    }

    export fn unwrap(w: Wrapper<i32>) -> i32 {
        return w.inner;
    }
}
"#;
    build_provider(&dir, "a", a, &sysroot);

    let b = r#"
import "a";

module beta {
    export fn make(v: i32) -> alpha::Wrapper<i32> {
        return alpha::wrap(v);
    }

    export fn take(w: alpha::Wrapper<i32>) -> i32 {
        return alpha::unwrap(w);
    }
}
"#;
    build_provider(&dir, "b", b, &sysroot);

    let c = r#"
import "b";

fn main() -> i32 {
    dec w = beta::make(42);
    if beta::take(w) == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "C must compile against B.llib");
    assert_eq!(code, 0, "A-owned generic instantiation must survive A -> B -> C");
}

// ---------------------------------------------------------------------------
// 4. Transitive trait method resolution: A -> B -> C.
// ---------------------------------------------------------------------------
#[test]
fn test_transitive_trait_method_resolution_a_b_c() {
    let dir = temp_dir("transitive_trait");
    let sysroot = sysroot_root();

    let a = r#"
module alpha {
    export trait Describer {
        fn describe(self: &Self) -> i32;
    }

    export struct Item {
        export x: i32,
    }

    impl Describer for Item {
        fn describe(self: &Self) -> i32 {
            return self.x;
        }
    }

    export fn make(x: i32) -> Item {
        return Item { x: x };
    }
}
"#;
    build_provider(&dir, "a", a, &sysroot);

    let b = r#"
import "a";

module beta {
    export fn describe_item(it: alpha::Item) -> i32 {
        return it.describe();
    }
}
"#;
    build_provider(&dir, "b", b, &sysroot);

    let c = r#"
import "b";

fn main() -> i32 {
    dec it = beta::make_item(42);
    if beta::describe_item(it) == 42 {
        return 0;
    }
    return 1;
}
"#;
    // beta exposes make_item re-exporting A's constructor.
    let b = r#"
import "a";

module beta {
    export fn make_item(x: i32) -> alpha::Item {
        return alpha::make(x);
    }

    export fn describe_item(it: alpha::Item) -> i32 {
        return it.describe();
    }
}
"#;
    // Rebuild B with the re-exported constructor.
    build_provider(&dir, "b", b, &sysroot);

    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "C must compile against B.llib");
    assert_eq!(code, 0, "trait method resolution must survive A -> B -> C");
}

// ---------------------------------------------------------------------------
// 5. Mixed source/artifact graph parity.
// ---------------------------------------------------------------------------
#[test]
fn test_mixed_source_artifact_graph_parity() {
    let sysroot = sysroot_root();

    let a = r#"
module alpha {
    export struct Item {
        export x: i32,
    }

    export fn make(x: i32) -> Item {
        return Item { x: x };
    }

    export fn read(it: &Item) -> i32 {
        return it.x;
    }
}
"#;
    let b = r#"
import "a";

module beta {
    export fn produce(v: i32) -> alpha::Item {
        return alpha::make(v);
    }

    export fn consume(it: alpha::Item) -> i32 {
        return alpha::read(&it);
    }
}
"#;
    let c = r#"
import "b";

fn main() -> i32 {
    dec it = beta::produce(42);
    if beta::consume(it) == 42 {
        return 0;
    }
    return 1;
}
"#;

    // Case A: A source, B artifact, C consumer.
    {
        let dir = temp_dir("mixed_a_src_b_art");
        fs::write(dir.join("a.ln"), a).unwrap();
        build_provider(&dir, "b", b, &sysroot);
        let (ok, code) = run_consumer(&dir, c, &sysroot);
        assert!(ok, "Case A source/artifact/consumer must compile");
        assert_eq!(code, 0, "Case A observable semantics must match");
    }

    // Case B: A artifact, B source, C consumer.
    {
        let dir = temp_dir("mixed_a_art_b_src");
        build_provider(&dir, "a", a, &sysroot);
        fs::write(dir.join("b.ln"), b).unwrap();
        let (ok, code) = run_consumer(&dir, c, &sysroot);
        assert!(ok, "Case B artifact/source/consumer must compile");
        assert_eq!(code, 0, "Case B observable semantics must match");
    }

    // Case C: A artifact, B artifact, C consumer.
    {
        let dir = temp_dir("mixed_a_art_b_art");
        build_provider(&dir, "a", a, &sysroot);
        build_provider(&dir, "b", b, &sysroot);
        let (ok, code) = run_consumer(&dir, c, &sysroot);
        assert!(ok, "Case C artifact/artifact/consumer must compile");
        assert_eq!(code, 0, "Case C observable semantics must match");
    }
}

// ---------------------------------------------------------------------------
// 6. Layer 1: direct SemanticMetadata inspection of a synthetic provider.
//    Proves generic-parameter types, reference mutability, and impl headers are
//    actually present in the serialized canonical interface.
// ---------------------------------------------------------------------------
#[test]
fn test_semantic_metadata_preserves_generic_refs_and_impls() {
    let dir = temp_dir("metadata_inspection");
    let sysroot = sysroot_root();

    let provider = r#"
module meta {
    export trait Marker {
        fn mark(self: &Self) -> i32;
    }

    export struct First<T> {
        export value: T,
    }

    export struct Second<T> {
        export value: T,
    }

    impl Marker for First<i32> {
        fn mark(self: &Self) -> i32 {
            return self.value;
        }
    }

    export fn keep<T>(x: T) -> T {
        return x;
    }

    export fn inspect(a: &i32) -> &i32 life_from(a) {
        return a;
    }

    export fn mutate(a: &rw i32) -> &rw i32 life_from(a) {
        return a;
    }
}
"#;
    build_provider(&dir, "meta", provider, &sysroot);

    let bytes = fs::read(dir.join("meta.llib")).unwrap();
    let mut cursor = std::io::Cursor::new(&bytes);
    let (_m, _manifest, _obj, semantic_opt) =
        MlibReader::read_module(&mut cursor).expect("read meta.llib");
    let semantic = semantic_opt.expect("SemanticMetadata missing from meta.llib");

    let root = semantic
        .interface
        .exported_symbols
        .get("meta")
        .expect("module meta must be exported");

    let first_gp = &root.children.get("First").expect("First export").generic_params;
    let second_gp = &root.children.get("Second").expect("Second export").generic_params;
    assert_eq!(first_gp.len(), 1, "First<T> must serialize exactly one generic param");
    assert_eq!(second_gp.len(), 1, "Second<T> must serialize exactly one generic param");
    assert_ne!(
        first_gp[0], second_gp[0],
        "same-name generic params in unrelated declarations must NOT collapse"
    );
    assert!(
        first_gp[0].symbol_path.contains("First"),
        "First's generic param identity must be scoped to its declaration: {:?}",
        first_gp[0]
    );
    assert!(
        second_gp[0].symbol_path.contains("Second"),
        "Second's generic param identity must be scoped to its declaration: {:?}",
        second_gp[0]
    );

    let types = &semantic.interface.types;
    assert!(
        types
            .iter()
            .any(|t| matches!(t, CanonicalType::GenericParam(_))),
        "generic parameter type must be serialized (via generic fn keep<T>)"
    );
    assert!(
        types
            .iter()
            .any(|t| matches!(t, CanonicalType::Reference(_, Mutability::Immutable, _))),
        "immutable reference type must be serialized"
    );
    assert!(
        types
            .iter()
            .any(|t| matches!(t, CanonicalType::Reference(_, Mutability::Mutable, _))),
        "mutable (&rw) reference type must be serialized distinctly"
    );
    assert!(
        !semantic.interface.impl_headers.is_empty(),
        "trait impl header must be serialized in the canonical interface"
    );

    assert!(root.children.contains_key("First"));
    assert!(root.children.contains_key("Second"));
    assert!(root.children.contains_key("inspect"));
    assert!(root.children.contains_key("mutate"));
}

// ---------------------------------------------------------------------------
// 7. Transitive lifetime contract: A -> B -> C.
// ---------------------------------------------------------------------------
#[test]
fn test_transitive_lifetime_contract_a_b_c() {
    let dir = temp_dir("transitive_lifetime");
    let sysroot = sysroot_root();

    let a = r#"
module alpha {
    export struct Item {
        export x: i32,
    }

    export fn make(x: i32) -> Item {
        return Item { x: x };
    }
}
"#;
    build_provider(&dir, "a", a, &sysroot);

    let b = r#"
import "a";

module beta {
    export fn make_item(x: i32) -> alpha::Item {
        return alpha::make(x);
    }

    export fn peek(it: &alpha::Item) -> &i32 life_from(it) {
        return &it.x;
    }
}
"#;
    build_provider(&dir, "b", b, &sysroot);

    let c = r#"
import "b";

fn main() -> i32 {
    dec it = beta::make_item(42);
    dec r = beta::peek(&it);
    if *r == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "C must compile against B.llib");
    assert_eq!(code, 0, "lifetime-bearing signature must survive A -> B -> C");
}

// ---------------------------------------------------------------------------
// 8. Generic trait bound declared in a provider survives .llib.
//    Provider declares `fn call_mark<T: Marker>(x: T)`; a consumer that only
//    sees the .llib must still instantiate it with its own Marker impl.
// ---------------------------------------------------------------------------
#[test]
fn test_provider_generic_trait_bound_survives_llib() {
    let dir = temp_dir("trait_bound");
    let sysroot = sysroot_root();

    let provider = r#"
module tb {
    export trait Marker {
        fn mark(self: &Self) -> i32;
    }

    export fn call_mark<T: Marker>(x: T) -> i32 {
        return x.mark();
    }
}
"#;
    build_provider(&dir, "tb", provider, &sysroot);

    let c = r#"
import "tb";

struct Thing {
    v: i32,
}

impl tb::Marker for Thing {
    fn mark(self: &Self) -> i32 {
        return self.v;
    }
}

fn main() -> i32 {
    dec t = Thing { v: 42 };
    if tb::call_mark(t) == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "consumer must compile against tb.llib");
    assert_eq!(code, 0, "provider-declared generic trait bound must survive .llib");
}

// ---------------------------------------------------------------------------
// 9. Associated type canonical .llib round-trip.
//    Provider declares a trait with an associated type, binds it in an impl,
//    and resolves a `T::Output` projection internally. Re-typechecking the
//    serialized AstInterface must still resolve the associated-type binding.
//
//    Note: cross-PROVIDER associated-type projection (`T::Output` on an
//    externally owned impl) is a pre-existing general language limitation that
//    fails identically on the SOURCE path, so it is out of scope for artifact
//    parity. This test uses the supported within-provider scope.
// ---------------------------------------------------------------------------
#[test]
fn test_associated_type_round_trip_canonical_llib() {
    let dir = temp_dir("assoc_type");
    let sysroot = sysroot_root();

    let provider = r#"
module assoc {
    export trait Producer {
        type Output;

        fn produce(self: &Self) -> Self::Output;
    }

    export struct Num {
        export v: i32,
    }

    impl Producer for Num {
        type Output = i32;

        fn produce(self: &Self) -> i32 {
            return self.v;
        }
    }

    fn output_of<T: Producer>(x: &T) -> T::Output {
        return x.produce();
    }

    export fn compute() -> i32 {
        dec n = Num { v: 42 };
        return output_of(&n);
    }

    export fn make(v: i32) -> Num {
        return Num { v: v };
    }
}
"#;
    build_provider(&dir, "assoc", provider, &sysroot);

    let c = r#"
import "assoc";

fn main() -> i32 {
    dec n = assoc::make(42);
    if assoc::compute() == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (ok, code) = run_consumer(&dir, c, &sysroot);
    assert!(ok, "consumer must compile against assoc.llib");
    assert_eq!(
        code, 0,
        "associated-type declaration + impl binding must survive canonical .llib"
    );
}
