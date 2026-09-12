macro add_one {
    (@x: expr) => {
        @x + 1
    }
}

macro add_two {
    (@x: expr) => {
        add_one!(add_one!(@x))
    }
}

fn main() -> i32 {
    dec val = add_two!(10);
    return val;
}
