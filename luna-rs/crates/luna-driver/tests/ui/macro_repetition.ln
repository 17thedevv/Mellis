macro make_array {
    (@x: expr) => { [@x] }
    (@x: expr, @y: expr) => { [@x, @y] }
    (@x: expr, @y: expr, @z: expr) => { [@x, @y, @z] }
}

fn main() -> i32 {
    dec arr = make_array![10, 20, 30];
    return arr[0];
}
