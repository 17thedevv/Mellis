struct Point {
    x: i32;
    y: i32;
}

fn main() {
    dec (a, b) = (1, 2);
    dec p = Point { x: 10, y: 20 };
    dec Point { x, y } = p;
}
