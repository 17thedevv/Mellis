// test_destructuring.ms
// Tests for destructuring patterns in let bindings and match

struct Point {
    x: i32;
    y: i32;
}

fn test_struct_destructuring() {
    dec p = Point { x: 10, y: 20 };
    dec Point { x, y } = p;
}

fn test_tuple_destructuring() {
    dec t: (int_32, int_32) = (10, 20);
    dec (a, b) = t;
}

fn main() {
    test_struct_destructuring();
    test_tuple_destructuring();
}
