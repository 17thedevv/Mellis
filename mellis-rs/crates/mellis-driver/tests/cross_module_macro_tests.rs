use mellis_driver::{check, compile, CompilerOptions};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("mellis_tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    
    // Mock core component since it's required for typechecking/bootstrapping
    let core_dir = dir.join("libs").join("external");
    fs::create_dir_all(&core_dir).unwrap();
    fs::write(core_dir.join("core.ms"), "module core {}").unwrap();
    
    dir
}

#[test]
fn test_cross_module_macro_export_and_import() {
    let dir = setup_test_dir("export_import");
    let prov_path = dir.join("math_mod.ms");
    let main_path = dir.join("main.ms");

    fs::write(
        &prov_path,
        r#"
    export macro add_ten {
        (@x: expr) => {
            @x + 10
        }
    }
    "#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
    import "math_mod";

    fn main() -> i32 {
        return add_ten!(5);
    }
    "#,
    )
    .unwrap();

    let main_src = fs::read_to_string(&main_path).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        no_link: true,
        search_paths: vec![dir.to_str().unwrap().to_string()],
        ..Default::default()
    };

    let check_res = check(
        main_path.to_str().unwrap(),
        main_src.clone(),
        &mellis_driver::CompilerOptions { search_paths: vec![dir.to_str().unwrap().to_string(), "../../libs/external".to_string()], ..Default::default() },
    );
    assert!(
        check_res.is_ok(),
        "Cross-module check failed: {:?}",
        check_res.err()
    );

    let compile_res = compile(main_path.to_str().unwrap(), main_src, &opts);
    assert!(
        compile_res.is_ok(),
        "Cross-module compile failed: {:?}",
        compile_res.err()
    );
}

#[test]
fn test_cross_module_macro_qualified_lookup() {
    let dir = setup_test_dir("qualified_lookup");
    let prov_path = dir.join("geometry.ms");
    let main_path = dir.join("main.ms");

    fs::write(
        &prov_path,
        r#"
    export module geom {
        export macro area {
            (@w: expr, @h: expr) => { @w * @h }
        }
    }
    "#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
    import "geometry";

    fn main() -> i32 {
        return geom::area!(6, 7);
    }
    "#,
    )
    .unwrap();

    let main_src = fs::read_to_string(&main_path).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        no_link: true,
        search_paths: vec![dir.to_str().unwrap().to_string()],
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), main_src, &opts);
    assert!(
        compile_res.is_ok(),
        "Qualified macro compile failed: {:?}",
        compile_res.err()
    );
}

#[test]
fn test_cross_module_macro_using_alias() {
    let dir = setup_test_dir("using_alias");
    let prov_path = dir.join("algebra.ms");
    let main_path = dir.join("main.ms");

    fs::write(
        &prov_path,
        r#"
    export module algebra {
        export macro square {
            (@x: expr) => { @x * @x }
        }
    }
    "#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
    import "algebra";
    using algebra as alg;

    fn main() -> i32 {
        return alg::square!(8);
    }
    "#,
    )
    .unwrap();

    let main_src = fs::read_to_string(&main_path).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        no_link: true,
        search_paths: vec![dir.to_str().unwrap().to_string()],
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), main_src, &opts);
    assert!(
        compile_res.is_ok(),
        "Using alias macro compile failed: {:?}",
        compile_res.err()
    );
}

#[test]
fn test_cross_module_macro_private_rejected() {
    let dir = setup_test_dir("private_rejected");
    let prov_path = dir.join("secret.ms");
    let main_path = dir.join("main.ms");

    fs::write(
        &prov_path,
        r#"
    macro private_calc {
        (@x: expr) => { @x * 2 }
    }
    "#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
    import "secret";

    fn main() -> i32 {
        return private_calc!(5);
    }
    "#,
    )
    .unwrap();

    let main_src = fs::read_to_string(&main_path).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        no_link: true,
        search_paths: vec![dir.to_str().unwrap().to_string()],
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), main_src, &opts);
    assert!(
        compile_res.is_err(),
        "Expected private macro to be rejected from consumer"
    );
    let errs = compile_res.err().unwrap();
    assert!(
        errs.iter().any(
            |d| d.message.contains("no macro named `private_calc` in scope")
                || d.message.contains("private_calc")
        ),
        "Unexpected error message: {:?}",
        errs
    );
}

#[test]
fn test_cross_module_hygiene_helper_resolution() {
    let dir = setup_test_dir("hygiene_helper");
    let prov_path = dir.join("crypto.ms");
    let main_path = dir.join("main.ms");

    fs::write(
        &prov_path,
        r#"
    export fn internal_helper(x: i32) -> i32 {
        return x * 3;
    }

    export macro calc_crypto {
        (@val: expr) => {
            internal_helper(@val)
        }
    }
    "#,
    )
    .unwrap();

    fs::write(
        &main_path,
        r#"
    import "crypto";

    fn main() -> i32 {
        return calc_crypto!(10);
    }
    "#,
    )
    .unwrap();

    let main_src = fs::read_to_string(&main_path).unwrap();
    let opts = CompilerOptions {
        quiet: true,
        no_link: true,
        search_paths: vec![dir.to_str().unwrap().to_string()],
        ..Default::default()
    };

    let compile_res = compile(main_path.to_str().unwrap(), main_src, &opts);
    assert!(
        compile_res.is_ok(),
        "Private helper hygiene compile failed: {:?}",
        compile_res.err()
    );
}
