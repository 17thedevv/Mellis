pub mod llvm_codegen;
pub mod linker;

pub use llvm_codegen::{LLVMBackend, TargetConfig, BackendError};
pub use linker::{compile_ll_to_exe, link_obj_to_exe, runtime_library};
