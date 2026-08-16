struct Point {
    value: str;
    x: int_32;
    y: int_32;
}

fn make_point() -> Point {
    return Point { value: "Hello", x: 1, y: 2 };
}

fn main() -> int_32 {
    dec Point { value, x, y } = make_point();
    
    // Explicit discard
    dec Point { value: val2, .. } = make_point();
    
    return x + y;
}
