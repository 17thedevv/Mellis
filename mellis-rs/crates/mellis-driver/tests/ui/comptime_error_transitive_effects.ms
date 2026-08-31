// Negative test: transitive effects (Extern) propagation from another pure-looking function

fn pure_looking() -> i32 {
    dec rw x = 5;
    x = x + 1;
    // Calling some hypothetical impure function
    impure_func()
}

extern fn impure_func() -> i32;

const FORBIDDEN: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    pure_looking()
};

fn main() -> i32 {
    FORBIDDEN
}
