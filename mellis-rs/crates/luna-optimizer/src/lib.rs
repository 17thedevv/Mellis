pub mod pass;
pub mod manager;
pub mod verifier;
pub mod passes {
    pub mod const_fold;
    pub mod dce;
}

pub use pass::Pass;
pub use manager::PassManager;
pub use verifier::{verify_module, verify_function};
pub use passes::const_fold::ConstantFolding;
pub use passes::dce::DeadCodeElimination;
