use mellis_mvir::Module;
use crate::pass::Pass;

pub struct PassManager {
    passes: Vec<Box<dyn Pass>>,
}

impl PassManager {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
        }
    }

    pub fn add_pass(&mut self, pass: Box<dyn Pass>) {
        self.passes.push(pass);
    }

    pub fn run(&mut self, module: &mut Module) {
        let mut changed = true;
        // Run passes until no changes are made (fixed-point iteration) or just run once per pass.
        // For simplicity right now, run each pass once. If we want fixed point, we can loop.
        for pass in &mut self.passes {
            let mut pass_changed = false;
            for func in &mut module.functions {
                if pass.run_on_function(func) {
                    pass_changed = true;
                }
            }
            if pass_changed {
                println!("Pass {} modified the module.", pass.name());
            }
        }
    }
}
