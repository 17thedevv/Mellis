macro single_arg {
    (@x: expr) => {
        @x + 1
    }
}

fn main() -> i32 {
    dec x: i32 = single_arg!(1, 2, 3); //~ ERROR no rule in macro `single_arg` matched the invocation arguments
    return x;
}
