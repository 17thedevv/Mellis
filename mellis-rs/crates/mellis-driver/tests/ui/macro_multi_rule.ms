macro compute {
    (@x: expr) => {
        @x + 10
    }
    (@x: expr, @y: expr) => {
        @x * @y
    }
}

fn main() -> i32 {
    dec a: i32 = compute!(5);
    dec b: i32 = compute!(3, 4);
    return a + b;
}
