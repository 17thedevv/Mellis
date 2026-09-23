macro single_arg {
    (@x: expr) => {
        @x + 1
    }
}

fn main() -> i32 {
    dec x: i32 = single_arg!(1, 2, 3); //~ ERROR expected end of macro invocation
    return x;
}
