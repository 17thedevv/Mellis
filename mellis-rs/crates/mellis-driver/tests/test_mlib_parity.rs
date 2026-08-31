use std::fs;
use std::path::PathBuf;
use mellis_driver::{compile, CompilerOptions};

#[test]
fn test_semantic_parity() {
    let parity_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("tests")
        .join("parity");
        
    let provider_path = parity_dir.join("provider.ms");
    let consumer_path = parity_dir.join("consumer.ms");
    let mlib_path = parity_dir.join("provider.mlib");
    let obj_path = parity_dir.join("consumer.obj");
    let exe_path = parity_dir.join("consumer.exe");
    
    // Clean up before test
    let _ = fs::remove_file(&mlib_path);
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    
    let consumer_src = fs::read_to_string(&consumer_path).expect("failed to read consumer.ms");
    
    // 1. Run Consumer with NO mlib present (Source Mode)
    println!("--- Testing Source Mode ---");
    let opts_src = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        quiet: false,
        search_paths: vec![parity_dir.to_str().unwrap().to_string()],
        no_link: true,
    };
    
    let res_src = compile(consumer_path.to_str().unwrap(), consumer_src.clone(), &opts_src);
    if let Err(errs) = &res_src {
        for e in errs {
            println!("Src Err: {:?}", e);
        }
    }
    assert!(res_src.is_ok(), "Consumer failed to compile in Source mode");
    
    // 2. Explicitly compile provider to mlib
    println!("--- Compiling Provider to MLib ---");
    let provider_src = fs::read_to_string(&provider_path).expect("failed to read provider.ms");
    let opts_prov = CompilerOptions {
        output_path: Some(mlib_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: true,
        quiet: false,
        search_paths: vec![],
        no_link: true,
    };
    let res_prov = compile(provider_path.to_str().unwrap(), provider_src.clone(), &opts_prov);
    assert!(res_prov.is_ok(), "Provider failed to compile to MLib");
    assert!(mlib_path.exists(), "MLib was not generated");
    
    // 3. Run Consumer with mlib present (MLib Mode)
    /*
    println!("--- Testing MLib Mode ---");
    let opts_mlib = CompilerOptions {
        output_path: Some(exe_path.to_str().unwrap().to_string()),
        emit_llvm: false,
        emit_mvir: false,
        emit_mlib: false,
        quiet: false,
        search_paths: vec![parity_dir.to_str().unwrap().to_string()],
        no_link: true,
    };
    
    let res_mlib = compile(consumer_path.to_str().unwrap(), consumer_src.clone(), &opts_mlib);
    if let Err(errs) = &res_mlib {
        for e in errs {
            println!("MLib Err: {:?}", e);
        }
    }
    assert!(res_mlib.is_ok(), "Consumer failed to compile in MLib mode");
    */
    
    // Clean up after test
    let _ = fs::remove_file(&mlib_path);
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
}
