use mellis_mvir::Function;

pub trait Pass {
    fn name(&self) -> &'static str;
    fn run_on_function(&mut self, func: &mut Function) -> bool;
}
