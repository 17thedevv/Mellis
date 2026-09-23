macro my_macro {
    (@x: expr) => {
        @x
    }
}

fn main() -> i32 {
    dec x = my_macro; //~ ERROR macro 'my_macro' cannot be used as a value
    return 0;
}
