// Negative test: stack pointer escaping comptime boundary is rejected

const ESCAPED: *i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    dec x = 42;
    &x
};

fn main() -> i32 {
    0
}
