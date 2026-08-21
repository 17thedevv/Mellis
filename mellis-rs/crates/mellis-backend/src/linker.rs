use std::{env, fs, path::PathBuf, process::Command};

fn tool(name: &str) -> PathBuf {
    env::var_os("LLVM_HOME")
        .map(PathBuf::from)
        .map(|root| root.join("bin").join(name))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

pub fn runtime_library() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(home) = env::var_os("MELLIS_HOME") {
        candidates.push(PathBuf::from(&home).join("runtime").join("mellis-runtime.lib"));
        candidates.push(PathBuf::from(&home).join("build").join("runtime").join("Release").join("mellis-runtime.lib"));
    }
    if let Some(path) = env::var_os("MELLIS_RUNTIME_LIB") {
        candidates.push(PathBuf::from(path));
    }
    
    if let Ok(exe_path) = env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            candidates.push(dir.join("mellis-runtime.lib"));
        }
    }
    
    candidates.push(PathBuf::from("runtime/mellis-runtime.lib"));
    candidates.push(PathBuf::from("D:\\fdlang\\build\\runtime\\Release\\mellis-runtime.lib"));

    candidates.into_iter().find(|path| path.is_file()).ok_or_else(||
        "mellis runtime library not found; set MELLIS_HOME or MELLIS_RUNTIME_LIB or place it alongside the compiler".into())
}

pub fn compile_ll_to_exe(ll_file: &str, obj_file: &str, exe_file: &str) -> Result<(), String> {
    // Call llc.exe
    let llc_status = Command::new(tool("llc.exe"))
        .arg("-filetype=obj")
        .arg("-o")
        .arg(obj_file)
        .arg(ll_file)
        .status()
        .map_err(|e| format!("Failed to invoke llc: {}", e))?;
        
    if !llc_status.success() {
        return Err(format!("llc exited with status {}", llc_status));
    }
    
    // Call clang.exe to link
    // Assuming MSVC environment or generic Clang
    let runtime = runtime_library()?;
    let clang_status = Command::new(tool("clang.exe"))
        .arg(obj_file)
        .arg(runtime)
        .arg("-o")
        .arg(exe_file)
        .status()
        .map_err(|e| format!("Failed to invoke clang: {}", e))?;
        
    if !clang_status.success() {
        return Err(format!("clang exited with status {}", clang_status));
    }
    
    // Cleanup temporary files
    let _ = fs::remove_file(ll_file);
    let _ = fs::remove_file(obj_file);
    
    Ok(())
}

pub fn link_obj_to_exe(obj_file: &str, exe_file: &str) -> Result<(), String> {
    let runtime = runtime_library()?;
    let clang_status = Command::new(tool("clang.exe"))
        .arg(obj_file)
        .arg(runtime)
        .arg("-o")
        .arg(exe_file)
        .status()
        .map_err(|e| format!("Failed to invoke clang: {}", e))?;
        
    if !clang_status.success() {
        return Err(format!("clang exited with status {}", clang_status));
    }
    
    // Optionally cleanup
    // let _ = fs::remove_file(obj_file);
    
    Ok(())
}
