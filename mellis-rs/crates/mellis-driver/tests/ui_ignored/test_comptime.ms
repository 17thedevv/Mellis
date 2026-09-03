comptime fn foo() -> i32 {
    return 42;
}

fn main() {
    dec x = comptime { foo() };
}
