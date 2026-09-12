use luna_driver::{check, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_visibility_tests").join(name);
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

#[test]
fn test_vis_struct_2_reject_export_field_in_private_struct() {
    let dir = setup_test_dir("vis_struct_2");
    let src = r#"
        struct Secret {
            export x: i32,
            y: i32,
        }

        fn main() -> i32 {
            return 0;
        }
    "#;

    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_err(), "Expected error declaring export field in private struct");
    let diags = res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Struct field cannot be declared 'export' in a private struct")),
        "Expected VIS-STRUCT-2 diagnostic, got: {:?}", diags
    );
}

#[test]
fn test_path_1_field_read() {
    let dir = setup_test_dir("path_1_read");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    fs::write(dir.join("auth_mod.ln"), prov).unwrap();

    // 1. Positive: Read public field
    let pass_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.name;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected reading public field to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Read private field
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.password;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected reading private field to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

#[test]
fn test_path_2_field_write() {
    let dir = setup_test_dir("path_2_write");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    fs::write(dir.join("auth_mod.ln"), prov).unwrap();

    // 1. Positive: Write public field
    let pass_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec rw u = create_user();
            u.name = 789;
            return u.name;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected writing public field to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Write private field
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec rw u = create_user();
            u.password = 999;
            return 0;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected writing private field to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

#[test]
fn test_path_3_field_borrow() {
    let dir = setup_test_dir("path_3_borrow");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    fs::write(dir.join("auth_mod.ln"), prov).unwrap();

    // 1. Positive: Borrow public field
    let pass_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            dec r: &i32 = &u.name;
            return *r;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected borrowing public field to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Shared borrow of private field
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            dec r = &u.password;
            return 0;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected borrowing private field to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );

    // 3. Negative: Mutable borrow of private field
    let fail_mut_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec rw u = create_user();
            dec rw r = &rw u.password;
            return 0;
        }
    "#;
    let fail_mut_res = check_source(&dir, "main_fail_mut.ln", fail_mut_src);
    assert!(fail_mut_res.is_err(), "Expected mutable borrowing of private field to fail");
    let mut_diags = fail_mut_res.err().unwrap();
    assert!(
        mut_diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", mut_diags
    );
}

#[test]
fn test_path_4_struct_literal_construction() {
    let dir = setup_test_dir("path_4_construction");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    fs::write(dir.join("auth_mod.ln"), prov).unwrap();

    // 1. Negative: External construction providing private field
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = User {
                name: 1,
                password: 2,
            };
            return 0;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected external struct construction with private field to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );

    // 2. Negative: External construction omitting private field
    let fail_omit_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = User {
                name: 1,
            };
            return 0;
        }
    "#;
    let fail_omit_res = check_source(&dir, "main_fail_omit.ln", fail_omit_src);
    assert!(fail_omit_res.is_err(), "Expected external struct construction omitting private field to fail");
    let omit_diags = fail_omit_res.err().unwrap();
    assert!(
        omit_diags.iter().any(|d| d.message.contains("Missing field `password`")),
        "Expected missing field diagnostic, got: {:?}", omit_diags
    );
}

#[test]
fn test_path_5_destructuring_match() {
    let dir = setup_test_dir("path_5_destructuring");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    fs::write(dir.join("auth_mod.ln"), prov).unwrap();

    // 1. Positive: Destructure public field with rest (..)
    let pass_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            match u {
                User { name, .. } -> {
                    return name;
                },
                _ -> {
                    return 0;
                }
            }
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected destructuring public field with .. to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Destructure private field
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            match u {
                User { password, .. } -> {
                    return password;
                },
                _ -> {
                    return 0;
                }
            }
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected destructuring private field to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

#[test]
fn test_path_6_nested_field_projection() {
    let dir = setup_test_dir("path_6_nested");
    let prov = r#"
        export struct Credential {
            export id: i32,
            secret: i32,
        }

        export struct Account {
            export cred: Credential,
        }

        export fn make_account() -> Account {
            return Account {
                cred: Credential {
                    id: 10,
                    secret: 999,
                },
            };
        }
    "#;
    fs::write(dir.join("nested_mod.ln"), prov).unwrap();

    // 1. Positive: Nested access to public field
    let pass_src = r#"
        import "nested_mod";

        fn main() -> i32 {
            dec a = make_account();
            return a.cred.id;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected nested public field access to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Nested access to private field
    let fail_src = r#"
        import "nested_mod";

        fn main() -> i32 {
            dec a = make_account();
            return a.cred.secret;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected nested private field access to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `secret` of struct `Credential` is private")),
        "Expected private field diagnostic, got: {:?}", diags
    );
}

#[test]
fn test_vis_struct_1_same_module_internal_access() {
    let dir = setup_test_dir("same_module");
    let src = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        fn check_password(u: &User) -> i32 {
            return u.password;
        }

        fn update_password(u: &rw User, new_pw: i32) {
            u.password = new_pw;
        }

        fn main() -> i32 {
            dec rw u = User {
                name: 1,
                password: 2,
            };
            update_password(&rw u, 3);
            return check_password(&u);
        }
    "#;

    let res = check_source(&dir, "main.ln", src);
    assert!(res.is_ok(), "Expected same module access to private fields to succeed, got: {:?}", res.err());
}

#[test]
fn test_compiled_mlib_parity() {
    let dir = setup_test_dir("mlib_parity");
    let prov = r#"
        export struct User {
            export name: i32,
            password: i32,
        }

        export fn create_user() -> User {
            return User {
                name: 123,
                password: 456,
            };
        }
    "#;
    let prov_path = dir.join("auth_mod.ln");
    fs::write(&prov_path, prov).unwrap();

    let mlib_path = dir.join("auth_mod.mlib");
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
    assert!(compile_prov.is_ok(), "Failed to compile auth_mod to mlib: {:?}", compile_prov.err());
    assert!(mlib_path.exists(), "auth_mod.mlib was not generated");

    // Remove the source file so consumer MUST resolve from .mlib!
    let _ = fs::remove_file(&prov_path);

    // 1. Positive: Read public field from .mlib
    let pass_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.name;
        }
    "#;
    let pass_res = check_source(&dir, "main_pass.ln", pass_src);
    assert!(pass_res.is_ok(), "Expected reading public field from mlib to succeed, got: {:?}", pass_res.err());

    // 2. Negative: Read private field from .mlib
    let fail_src = r#"
        import "auth_mod";

        fn main() -> i32 {
            dec u = create_user();
            return u.password;
        }
    "#;
    let fail_res = check_source(&dir, "main_fail.ln", fail_src);
    assert!(fail_res.is_err(), "Expected reading private field from mlib to fail");
    let diags = fail_res.err().unwrap();
    assert!(
        diags.iter().any(|d| d.message.contains("Field `password` of struct `User` is private")),
        "Expected private field diagnostic from mlib, got: {:?}", diags
    );
}
