trait Display {
    fn display(self: *Self) -> void;
}

impl Display for int_32 {
    fn display(self: *Self) -> void {
    }
}

// Generic function with lifetime and trait bound
fn identity<'a, T: Display>(x: &'a T) -> &'a T {
    return x;
}

fn main() -> int_32 {
    dec val: int_32 = 42;
    dec r: &'static int_32 = &val;
    dec r2 = identity@<'static, int_32>(r);
    return 0;
}
