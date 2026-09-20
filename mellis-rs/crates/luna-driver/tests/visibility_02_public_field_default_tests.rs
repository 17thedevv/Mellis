// =============================================================================
// Visibility-02 — Public Field Default
// Acceptance Tests
//
// RFC: .agents/skills/luna-semantic-compliance/references/visibility-02-public-field-default-rfc.md
//
// Canonical Rule:
//   Field visibility declared independently from struct visibility.
//   unmodified → Public, export → Public, private → Private
//
//   Access check: first validate containing struct accessibility,
//   then validate field accessibility.
//
// These tests cover TC-VIS-02-1 through TC-VIS-02-8
// =============================================================================

use luna_driver::{check, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_vis02_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let core_dir = dir.join("libs").join("external");
    fs::create_dir_all(&core_dir).unwrap();
    fs::write(core_dir.join("core.ln"), "module core {}").unwrap();

    dir
}

fn check_source(dir: &PathBuf, file_name: &str, src: &str) -> Result<(), Vec<luna_common::Diagnostic>> {
    let file_path = dir.join(file_name);
    fs::write(&file_path, src).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        search_paths: vec![dir.to_str().unwrap().to_string(), "../../libs/external".to_string()],
        ..Default::default()
    };
    check(file_path.to_str().unwrap(), src.to_string(), &opts)
}

// =============================================================================
// TC-VIS-02-1: Public Struct + Implicit Public Fields
// Fields without explicit modifier are Public when struct is accessible
// =============================================================================

#[test]
fn test_tc_vis_02_1_implicit_public_field_access() {
    let dir = setup_test_dir("tc_vis_02_1");

    let prov = r#"
        export struct Point {
            x: i32,
            y: i32,
        };

        export fn create_point() -> Point {
            return Point { x: 1, y: 2 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    // External can read implicit public fields
    let src = r#"
        import "geom";

        fn main() -> i32 {
            dec p = create_point();
            return p.x + p.y;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected implicit public field read to succeed, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_1_implicit_public_field_write() {
    let dir = setup_test_dir("tc_vis_02_1_write");

    let prov = r#"
        export struct Point {
            x: i32,
            y: i32,
        };

        export fn create_point() -> Point {
            return Point { x: 1, y: 2 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    // External can write implicit public fields
    let src = r#"
        import "geom";

        fn main() -> i32 {
            dec rw p = create_point();
            p.x = 10;
            p.y = 20;
            return p.x + p.y;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected implicit public field write to succeed, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_1_implicit_public_field_borrow() {
    let dir = setup_test_dir("tc_vis_02_1_borrow");

    let prov = r#"
        export struct Point {
            x: i32,
            y: i32,
        };

        export fn create_point() -> Point {
            return Point { x: 1, y: 2 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    // External can borrow implicit public fields
    let src = r#"
        import "geom";

        fn use_borrow(r: &i32) -> i32 {
            return *r;
        }

        fn main() -> i32 {
            dec p = create_point();
            return use_borrow(&p.x) + use_borrow(&p.y);
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected implicit public field borrow to succeed, got: {:?}", res.err());
}

// =============================================================================
// TC-VIS-02-2: Public Struct + Explicit Private Fields
// Mix of implicit public and explicit private fields
// =============================================================================

#[test]
fn test_tc_vis_02_2_explicit_private_field_rejected() {
    let dir = setup_test_dir("tc_vis_02_2");

    let prov = r#"
        export struct BankAccount {
            balance: i32,
            private pin: u32,
        };

        export fn create_account() -> BankAccount {
            return BankAccount { balance: 1000, pin: 1234 as u32 };
        }
    "#;
    fs::write(dir.join("bank.ln"), prov).unwrap();

    // External can read public field
    let pass_src = r#"
        import "bank";

        fn main() -> i32 {
            dec acc = create_account();
            return acc.balance as i32;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected public field read to succeed, got: {:?}", pass_res.err());

    // External cannot read private field
    let fail_src = r#"
        import "bank";

        fn main() -> i32 {
            dec acc = create_account();
            return acc.pin as i32;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected private field read to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `pin` of struct `BankAccount` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

// =============================================================================
// TC-VIS-02-3: Private Struct + Fields
// Field visibility remains independent from struct visibility
// A public field in a private struct is still declared Public; inaccessible due to struct gate
// =============================================================================

#[test]
fn test_tc_vis_02_3_private_struct_internal_access() {
    let dir = setup_test_dir("tc_vis_02_3");

    // Same module: private struct fields accessible within module
    let src = r#"
        struct Internal {
            x: i32,
            y: i32,
        };

        fn get_internal() -> Internal {
            return Internal { x: 1, y: 2 };
        }

        fn main() -> i32 {
            dec i = get_internal();
            return i.x + i.y;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected same-module access to private struct fields, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_3_explicit_private_in_private_struct() {
    let dir = setup_test_dir("tc_vis_02_3_explicit");

    // Private struct with explicit private field (redundant but valid)
    let src = r#"
        struct Config {
            name: str,
            private secret: str,
        };

        fn get_config() -> Config {
            return Config { name: "app", secret: "xyz" };
        }

        fn main() -> i32 {
            dec c = get_config();
            return 0;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected private struct with explicit private field, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_3_public_field_in_private_struct_external_rejected() {
    // Test that external access to a private struct fails
    // This uses inline module to ensure visibility boundaries are enforced
    let dir = setup_test_dir("tc_vis_02_3_external");
    let src = r#"
        module secret {
            struct Secret {
                value: i32,           // declared Public, but struct is private
                private hidden: str, // declared Private
            };

            export fn create_secret() -> Secret {
                return Secret { value: 42, hidden: "secret" };
            }

            module external {
                fn try_access() -> i32 {
                    dec s = secret::create_secret();
                    // This should fail - Secret struct is private
                    // so even its "public" fields are inaccessible
                    return s.value;
                }
            }

            export fn test() -> i32 {
                return external::try_access();
            }
        }

        fn main() -> i32 {
            return secret::test();
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_err(), "Expected external module access to private struct to fail");
    let diags = res.err().unwrap();
    // Error should be about struct or field visibility
    assert!(
        diags.iter().any(|d| d.message.contains("private") || d.message.contains("struct")),
        "Expected visibility diagnostic, got: {:?}", diags
    );
}

// =============================================================================
// TC-VIS-02-4: Nested Module Access
// Descendant modules can access fields of ancestor private structs
// =============================================================================

#[test]
fn test_tc_vis_02_4_nested_module_access() {
    let dir = setup_test_dir("tc_vis_02_4");

    let src = r#"
        module outer {
            struct OuterData {
                value: i32,
            };

            module inner {
                export fn get_value() -> i32 {
                    dec d = OuterData { value: 42 };
                    return d.value;
                }
            }

            export fn test_outer() -> i32 {
                return inner::get_value();
            }
        }

        fn main() -> i32 {
            return 0;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected nested module access to ancestor struct fields, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_4_cross_module_access_public_field() {
    // Test that sibling modules CAN access public fields of each other's structs
    let dir = setup_test_dir("tc_vis_02_4_sibling");

    let src = r#"
        module outer {
            module mod_a {
                export struct TypeA {
                    value: i32,
                    private secret: i32,
                };

                export fn create_a() -> TypeA {
                    return TypeA { value: 1, secret: 2 };
                }
            }

            export module mod_b {
                export fn access_public() -> i32 {
                    dec a = mod_a::create_a();
                    return a.value;  // OK: public field
                }
            }
        }

        fn main() -> i32 {
            return outer::mod_b::access_public();
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected public field access from sibling module to succeed, got: {:?}", res.err());
}

// =============================================================================
// TC-VIS-02-5: Source/.llib Parity
// Field visibility defaults must match between .ln and .llib
// =============================================================================

#[test]
fn test_tc_vis_02_5_source_mlib_parity() {
    let dir = setup_test_dir("tc_vis_02_5_parity");

    let prov = r#"
        export struct Data {
            x: i32,
            y: i32,
            private z: i32,
        };

        export fn create_data() -> Data {
            return Data { x: 1, y: 2, z: 3 };
        }
    "#;
    let prov_path = dir.join("data_mod.ln");
    fs::write(&prov_path, prov).unwrap();

    // Compile to .mlib
    let mlib_path = dir.join("data_mod.mlib");
    let opts_prov = CompilerOptions {
        output_path: Some(mlib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: true,
        search_paths: vec![dir.to_str().unwrap().to_string(), "../../libs/external".to_string()],
        no_link: true,
        ..Default::default()
    };
    let compile_prov = luna_driver::compile(prov_path.to_str().unwrap(), prov.to_string(), &opts_prov);
    assert!(compile_prov.is_ok(), "Failed to compile to mlib: {:?}", compile_prov.err());
    assert!(mlib_path.exists(), "data_mod.mlib was not generated");

    // Remove source, consumer MUST use .mlib
    let _ = fs::remove_file(&prov_path);

    // 1. Positive: Read implicit public field from .mlib
    let pass_src = r#"
        import "data_mod";

        fn main() -> i32 {
            dec d = create_data();
            return d.x + d.y;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected public field read from mlib to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Read explicit private field from .mlib
    let fail_src = r#"
        import "data_mod";

        fn main() -> i32 {
            dec d = create_data();
            return d.z;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected private field read from mlib to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `z` of struct `Data` is private")),
        "Expected private field diagnostic from mlib, got: {:?}", diags
    );
}

// =============================================================================
// TC-VIS-02-6: Pattern/Destructuring
// Implicit public fields work with destructuring
// =============================================================================

#[test]
fn test_tc_vis_02_6_destructuring_implicit_public() {
    let dir = setup_test_dir("tc_vis_02_6");

    let prov = r#"
        export struct Point {
            x: i32,
            y: i32,
        };

        export fn origin() -> Point {
            return Point { x: 0, y: 0 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    // Destructuring implicit public fields - note: Luna uses `dec`, not `let`
    let src = r#"
        import "geom";

        fn main() -> i32 {
            dec p = origin();
            dec Point { x, y } = p;
            return x + y;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected destructuring of implicit public fields, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_6_destructuring_with_rest() {
    let dir = setup_test_dir("tc_vis_02_6_rest");

    let prov = r#"
        export struct Point {
            x: i32,
            y: i32,
            private z: i32,
        };

        export fn create() -> Point {
            return Point { x: 1, y: 2, z: 3 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    // Destructuring with .. ignores private fields
    let src = r#"
        import "geom";

        fn main() -> i32 {
            dec p = create();
            dec Point { x, .. } = p;
            return x;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected destructuring with rest pattern, got: {:?}", res.err());
}

// =============================================================================
// TC-VIS-02-7: Field Borrow/Read/Write (All 6 Access Paths)
// All VIS-STRUCT-6 access paths work for implicit public fields
// =============================================================================

#[test]
fn test_tc_vis_02_7_all_access_paths() {
    let dir = setup_test_dir("tc_vis_02_7");

    let prov = r#"
        export struct Data {
            value: i32,
        };

        export fn create() -> Data {
            return Data { value: 42 };
        }

        export fn read_value(d: &Data) -> i32 {
            return d.value;
        }

        export fn write_value(d: &rw Data, v: i32) {
            d.value = v;
        }
    "#;
    fs::write(dir.join("data.ln"), prov).unwrap();

    // Access path 1-3: read, write, borrow
    let src = r#"
        import "data";

        fn main() -> i32 {
            dec rw d = create();

            // 1. Read
            dec v1 = d.value;

            // 2. Write
            d.value = 100;

            // 3. Borrow (shared)
            dec r = &d.value;

            // 3. Borrow (mutable)
            dec rw mr = &rw d.value;

            return v1 + d.value;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected all basic access paths to succeed, got: {:?}", res.err());
}

#[test]
fn test_tc_vis_02_7_nested_projection() {
    let dir = setup_test_dir("tc_vis_02_7_nested");

    let prov = r#"
        export struct Inner {
            x: i32,
        };

        export struct Outer {
            inner: Inner,
        };

        export fn create() -> Outer {
            return Outer { inner: Inner { x: 1 } };
        }
    "#;
    fs::write(dir.join("nested.ln"), prov).unwrap();

    // Access path 6: Nested field projection
    let src = r#"
        import "nested";

        fn main() -> i32 {
            dec o = create();
            // Nested projection: o.inner.x
            return o.inner.x;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected nested field projection to succeed, got: {:?}", res.err());
}

// =============================================================================
// TC-VIS-02-8: Public Struct + Private Field Blocks External Construction
// Critical test: field read visibility ≠ field construction visibility
// =============================================================================

#[test]
fn test_tc_vis_02_8_external_construction_requires_all_fields() {
    let dir = setup_test_dir("tc_vis_02_8");

    let prov = r#"
        export struct Config {
            name: str,
            private secret: str,
        };
    "#;
    fs::write(dir.join("config_mod.ln"), prov).unwrap();

    // External: omitting private field fails
    let fail_omit_src = r#"
        import "config_mod";

        fn main() -> i32 {
            dec c = Config { name: "app" };
            return 0;
        }
    "#;
    let fail_omit_res = check_source(&dir, "main_fail_omit.ln", fail_omit_src);
    assert!(fail_omit_res.is_err(), "Expected external construction omitting private field to fail");
    let omit_diags = fail_omit_res.err().unwrap();
    assert!(
        omit_diags.iter().any(|d| d.message.to_lowercase().contains("missing") || d.message.contains("Required")),
        "Expected missing field diagnostic, got: {:?}", omit_diags
    );

    // External: providing private field also fails
    let fail_provide_src = r#"
        import "config_mod";

        fn main() -> i32 {
            dec c = Config { name: "app", secret: "xyz" };
            return 0;
        }
    "#;
    let fail_provide_res = check_source(&dir, "main_fail_provide.ln", fail_provide_src);
    assert!(fail_provide_res.is_err(), "Expected external construction with private field to fail");
    let provide_diags = fail_provide_res.err().unwrap();
    assert!(
        provide_diags.iter().any(|d| d.message.contains("Field `secret`") && d.message.contains("private")),
        "Expected private field diagnostic, got: {:?}", provide_diags
    );
}

#[test]
fn test_tc_vis_02_8_constructor_function_provides_private() {
    let dir = setup_test_dir("tc_vis_02_8_ctor");

    let prov = r#"
        export struct Config {
            name: str,
            private secret: str,
        };

        export fn create_config(n: str) -> Config {
            return Config { name: n, secret: "default" };
        }
    "#;
    fs::write(dir.join("config_mod.ln"), prov).unwrap();

    // External: use factory function that provides private field
    let src = r#"
        import "config_mod";

        fn main() -> i32 {
            dec c = create_config("app");
            dec visible_name: str = c.name;
            return 0;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected factory function usage to succeed, got: {:?}", res.err());

    // External: cannot read private field even with factory
    let fail_src = r#"
        import "config_mod";

        fn main() -> i32 {
            dec c = create_config("app");
            return c.secret.len();
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected private field read to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `secret`") && d.message.contains("private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

// =============================================================================
// Backward Compatibility: Explicit `export` still works
// =============================================================================

#[test]
fn test_backward_compat_explicit_export() {
    let dir = setup_test_dir("backward_compat");

    // Old style with explicit export should still work
    let prov = r#"
        export struct Point {
            export x: i32,
            export y: i32,
        };

        export fn create() -> Point {
            return Point { x: 1, y: 2 };
        }
    "#;
    fs::write(dir.join("geom.ln"), prov).unwrap();

    let src = r#"
        import "geom";

        fn main() -> i32 {
            dec p = create();
            return p.x + p.y;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected explicit export to still work, got: {:?}", res.err());
}

// =============================================================================
// Edge Case: Mixed visibility in same struct
// =============================================================================

#[test]
fn test_mixed_visibility() {
    let dir = setup_test_dir("mixed_vis");

    let prov = r#"
        export struct User {
            name_len: i32,           // implicit public
            email_len: i32,          // implicit public
            export id: u64,          // explicit public
            private password_hash: i32, // explicit private
        };

        export fn create_user() -> User {
            return User {
                name_len: 5,
                email_len: 15,
                id: 1 as u64,
                password_hash: 12345,
            };
        }
    "#;
    fs::write(dir.join("user_mod.ln"), prov).unwrap();

    // Can access all non-private fields
    let src = r#"
        import "user_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.name_len + u.email_len + (u.id as i32);
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected mixed visibility to work, got: {:?}", res.err());

    // Cannot access private field
    let fail_src = r#"
        import "user_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.password_hash;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected private field to be rejected");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password_hash` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

// =============================================================================
// VIS-STRUCT-2 Removal Test: Private struct CAN have "public" fields
// This was previously rejected but now allowed under Visibility-02
// =============================================================================

#[test]
fn test_vis_struct_2_removed_private_struct_public_field_allowed() {
    let dir = setup_test_dir("vis_struct_2_removed");

    // Previously this was rejected: private struct with export field
    // Now it's allowed because:
    // 1. Field is declared Public
    // 2. But struct itself is private, so external access fails at struct gate
    let src = r#"
        struct Inner {
            export value: i32,
        };

        fn get_inner() -> Inner {
            return Inner { value: 42 };
        }

        fn main() -> i32 {
            dec i = get_inner();
            return i.value;
        }
    "#;
    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected private struct with public field to compile, got: {:?}", res.err());
}
