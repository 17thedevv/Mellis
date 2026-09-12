use luna_driver::{check, compile, CompilerOptions};
use luna_driver::sysroot::Sysroot;
use std::fs;
use std::path::{Path, PathBuf};

fn create_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("luna_core_ptr_mem_acceptance_tests")
        .join(test_name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test temp dir");
    dir
}

fn locate_canonical_core_ln() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let core_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("libs")
        .join("external")
        .join("core.ln");
    assert!(core_path.exists(), "libs/external/core.ln must exist");
    core_path
}

fn make_opts(sysroot: &Sysroot, extra_path: Option<&Path>) -> CompilerOptions {
    let mut search_paths = vec![sysroot.root().to_string_lossy().to_string()];
    if let Some(extra) = extra_path {
        search_paths.insert(0, extra.to_string_lossy().to_string());
    }
    CompilerOptions {
        search_paths,
        quiet: true,
        no_link: true,
        ..Default::default()
    }
}

/// P1 & P2: `import <core>; ptr::read(...)` and `ptr::write(...)` succeed in unsafe context.
#[test]
fn test_p1_p2_ptr_read_write() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p1_p2_read_write");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() -> i32 {
            dec x: i32 = 42;
            dec rw y: i32 = 0;
            unsafe {
                dec val = ptr::read<i32>(&x as *i32);
                ptr::write<i32>(&rw y as *rw i32, val);
            }
            return y;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "ptr::read and ptr::write must succeed: {:?}", res.err());
}

/// P3: `core::ptr::read(...)` MUST be rejected (Provider != Namespace invariant, Rule 7).
#[test]
fn test_p3_core_ptr_namespace_rejected() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p3_provider_as_namespace");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() {
            dec x: i32 = 42;
            unsafe {
                dec val = core::ptr::read<i32>(&x as *i32);
            }
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "core::ptr::read MUST be rejected as Provider != Namespace");
}

/// P4: `import <core>; mem::copy(...)`, `mem::set(...)`, and `mem::zero(...)` succeed.
#[test]
fn test_p4_mem_primitives() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p4_mem_primitives");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() {
            dec src_byte: u8 = 255 as u8;
            dec rw dst_byte: u8 = 0 as u8;
            dec src_ptr = &src_byte as *u8;
            dec dst_ptr = &rw dst_byte as *rw u8;
            unsafe {
                mem::copy(src_ptr, dst_ptr, 1 as u64);
                mem::set(dst_ptr, 128 as u8, 1 as u64);
                mem::zero(dst_ptr, 1 as u64);
            }
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "mem::copy, mem::set, and mem::zero must succeed: {:?}", res.err());
}

/// P5: Calling raw pointer APIs outside `unsafe` block MUST be rejected.
#[test]
fn test_p5_unsafe_enforcement() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p5_unsafe_enforcement");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() {
            dec x: i32 = 42;
            dec val = ptr::read<i32>(&x as *i32);
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_err(), "ptr::read outside unsafe block must be rejected");
    let errs = res.unwrap_err();
    assert!(errs.iter().any(|d| d.message.to_lowercase().contains("unsafe")),
        "Expected unsafe diagnostic, got: {:?}", errs);
}

/// P6 & P7: Pointer arithmetic (`ptr::add`, `ptr::offset`, `ptr::diff`).
#[test]
fn test_p6_p7_ptr_arithmetic_and_diff() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p6_p7_arithmetic");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn main() -> i32 {
            dec a: i32 = 10;
            dec b: i32 = 20;
            dec p_a = &a as *i32;
            dec p_b = &b as *i32;
            unsafe {
                dec p_next = ptr::add<i32>(p_a, 1 as u64);
                dec p_back = ptr::offset<i32>(p_next, -1 as i64);
                dec distance = ptr::diff<i32>(p_b, p_a);
                return distance as i32;
            }
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "ptr::add, ptr::offset, and ptr::diff must succeed: {:?}", res.err());
}

/// P9: `ptr::read` does not synthesize safe borrow provenance (PTR-MEM-1).
#[test]
fn test_p9_ptr_read_does_not_infer_lifetime() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p9_no_lifetime_inference");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        fn escape_val() -> i32 {
            dec local: i32 = 999;
            unsafe {
                // Reading an owned value from raw pointer does NOT borrow local
                return ptr::read<i32>(&local as *i32);
            }
        }
        fn main() {
            dec v = escape_val();
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "ptr::read must produce an owned value without lifetime escape error: {:?}", res.err());
}

/// P11: Generic `T` with user-defined struct and `ptr::copy`.
#[test]
fn test_p11_generic_struct_ptr_copy() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p11_generic_struct");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        export struct Point {
            export x: i32,
            export y: i32,
        }
        fn main() {
            dec p1 = Point { x: 10, y: 20 };
            dec rw p2 = Point { x: 0, y: 0 };
            unsafe {
                ptr::copy<Point>(&p1 as *Point, &rw p2 as *rw Point, 1 as u64);
                dec read_back = ptr::read<Point>(&p2 as *Point);
            }
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "ptr::copy and ptr::read on struct Point must succeed: {:?}", res.err());
}

/// P14 & P15: `mem::size_of` and `mem::align_of` layout queries.
#[test]
fn test_p14_p15_size_of_align_of() {
    let sysroot = Sysroot::discover_for_test().expect("sysroot required");
    let dir = create_temp_dir("p14_p15_layout");
    let main_path = dir.join("main.ln");
    let src = r#"
        import <core>;
        export struct Packet {
            export header: u32,
            export payload: u64,
        }
        fn main() -> i32 {
            dec sz_i32 = mem::size_of<i32>();
            dec sz_pkt = mem::size_of<Packet>();
            dec al_pkt = mem::align_of<Packet>();
            return (sz_i32 + sz_pkt + al_pkt) as i32;
        }
    "#;
    fs::write(&main_path, src).unwrap();
    let opts = make_opts(&sysroot, Some(&dir));
    let res = check(main_path.to_str().unwrap(), src.to_string(), &opts);
    assert!(res.is_ok(), "mem::size_of and mem::align_of must succeed: {:?}", res.err());
}

/// P8: Source `.ln` vs binary `.llib` parity (PTR-MEM-5).
#[test]
fn test_p8_source_vs_llib_parity() {
    let test_sysroot = Sysroot::discover_for_test().expect("Failed to locate test sysroot");
    let dir = create_temp_dir("p8_parity");

    let core_path = locate_canonical_core_ln();
    let core_src = fs::read_to_string(&core_path).expect("Failed to read core.ln");

    let core_llib = dir.join("core.llib");
    let compile_opts = CompilerOptions {
        output_path: Some(core_llib.to_string_lossy().to_string()),
        emit_mlib: true,
        no_link: true,
        quiet: false,
        search_paths: vec![test_sysroot.root().to_string_lossy().to_string()],
        ..Default::default()
    };

    let res_compile = compile(core_path.to_str().unwrap(), core_src, &compile_opts);
    assert!(
        res_compile.is_ok(),
        "Compiling core.ln with ptr/mem to core.llib must succeed: {:?}",
        res_compile.err()
    );
    assert!(core_llib.exists(), "core.llib artifact must exist on disk");

    // Sync canonical libs/external/core.llib with freshly validated build
    let canonical_llib = core_path.with_file_name("core.llib");
    let _ = fs::copy(&core_llib, &canonical_llib);

    let mut file = fs::File::open(&canonical_llib).expect("Failed to open canonical core.llib");
    let (_, _, _, sem_opt) = luna_llib::MlibReader::read_module(&mut file).expect("Failed to read canonical core.llib");
    if let Some(sem) = sem_opt {
        println!("CANONICAL CORE.LLIB EXPORTED SYMBOLS: {:?}", sem.interface.exported_symbols.keys().collect::<Vec<_>>());
        if let Some(ptr_sym) = sem.interface.exported_symbols.get("ptr") {
            println!("PTR SYMBOL CHILDREN: {:?}", ptr_sym.children.keys().collect::<Vec<_>>());
        } else {
            println!("PTR SYMBOL NOT IN EXPORTED_SYMBOLS!");
        }
    } else {
        println!("NO SEMANTIC METADATA IN CORE.LLIB!");
    }

    // Test consumer against compiled binary library
    let main_path = dir.join("consumer.ln");
    let consumer_src = r#"
        import <core>;
        fn main() -> i32 {
            dec x: i32 = 100;
            dec rw y: i32 = 0;
            unsafe {
                dec val = ptr::read<i32>(&x as *i32);
                ptr::write<i32>(&rw y as *rw i32, val + 1);
            }
            return y;
        }
    "#;
    fs::write(&main_path, consumer_src).unwrap();

    let consumer_opts = CompilerOptions {
        search_paths: vec![dir.to_string_lossy().to_string()],
        quiet: false,
        no_link: true,
        ..Default::default()
    };
    let res_consumer = check(main_path.to_str().unwrap(), consumer_src.to_string(), &consumer_opts);
    assert!(
        res_consumer.is_ok(),
        "Consumer importing <core> binary .llib with ptr/mem must succeed: {:?}",
        res_consumer.err()
    );
}
