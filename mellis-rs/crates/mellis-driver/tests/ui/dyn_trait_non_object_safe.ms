trait NonSafeGeneric {
    fn generic_fn<T>(self: &Self, val: T);
}

trait NonSafeStatic {
    fn make_new() -> i32;
}

trait NonSafeSelfReturn {
    fn clone_self(self: &Self) -> Self;
}

fn test_generic(x: &dyn NonSafeGeneric) {} //~ ERROR Trait `NonSafeGeneric` cannot be made into an object because method `generic_fn` has generic type parameters
fn test_static(y: &dyn NonSafeStatic) {} //~ ERROR Trait `NonSafeStatic` cannot be made into an object because method `make_new` has no `self` receiver
fn test_self_ret(z: &dyn NonSafeSelfReturn) {} //~ ERROR Trait `NonSafeSelfReturn` cannot be made into an object because method `clone_self` returns `Self`

fn main() {}
