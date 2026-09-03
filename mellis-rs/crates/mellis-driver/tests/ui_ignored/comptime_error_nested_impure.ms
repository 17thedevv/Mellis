// Negative test: nested impure calls should propagate effect and be rejected at comptime
extern fn puts(s: *u8) -> i32;

fn nested_call() -> i32 {
    puts(cast(*u8) 0)
}

fn wrapper() -> i32 {
    nested_call()
}

const BAD: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    wrapper()
};

fn main() -> i32 {
    BAD
}
