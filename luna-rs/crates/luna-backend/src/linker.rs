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
    if let Some(home) = env::var_os("LUNA_HOME") {
        candidates.push(PathBuf::from(&home).join("runtime").join("luna-runtime.lib"));
        candidates.push(PathBuf::from(&home).join("build").join("runtime").join("Release").join("luna-runtime.lib"));
    }
    if let Some(path) = env::var_os("LUNA_RUNTIME_LIB") {
        candidates.push(PathBuf::from(path));
    }
    
    if let Ok(exe_path) = env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            candidates.push(dir.join("luna-runtime.lib"));
        }
    }
    
    candidates.push(PathBuf::from("runtime/luna-runtime.lib"));
    candidates.push(PathBuf::from("D:\\fdlang\\runtime\\luna-runtime.lib"));
    candidates.push(PathBuf::from("D:\\fdlang\\build\\runtime\\Release\\luna-runtime.lib"));

    candidates.into_iter().find(|path| path.is_file()).ok_or_else(||
        "luna runtime library not found; set LUNA_HOME or LUNA_RUNTIME_LIB or place it alongside the compiler".into())
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
    
    // Call gcc.exe to link
    // Assuming MinGW environment
    let runtime = runtime_library()?;
    let gcc_status = Command::new(tool("gcc.exe"))
        .arg(obj_file)
        .arg(runtime)
        .arg("-o")
        .arg(exe_file)
        // Optionally try ASan:
        // .arg("-fsanitize=address")
        .status()
        .map_err(|e| format!("Failed to invoke gcc: {}", e))?;
        
    if !gcc_status.success() {
        return Err(format!("gcc exited with status {}", gcc_status));
    }
    
    // Cleanup temporary files
    let _ = fs::remove_file(ll_file);
    let _ = fs::remove_file(obj_file);
    
    Ok(())
}

pub fn link_objs_to_exe<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(obj_files: &[P], exe_file: Q) -> Result<(), String> {
    let runtime = runtime_library()?;
    let mut cmd = Command::new(tool("gcc.exe"));
    for obj in obj_files {
        cmd.arg(obj.as_ref());
    }
    cmd.arg(runtime)
        .arg("-o")
        .arg(exe_file.as_ref());

    let output = cmd.output()
        .map_err(|e| format!("Failed to invoke gcc: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gcc exited with status {}: {}", output.status, stderr));
    }

    Ok(())
}

pub fn link_obj_to_exe(obj_file: &str, exe_file: &str) -> Result<(), String> {
    link_objs_to_exe(&[std::path::Path::new(obj_file)], std::path::Path::new(exe_file))
}
