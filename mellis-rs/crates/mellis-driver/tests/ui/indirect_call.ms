fn add(x: i32, y: i32) -> i32 {
    return x + y;
}

fn apply(f: fn(i32, i32) -> i32, a: i32, b: i32) -> i32 {
    return f(a, b);
}

fn main() -> i32 {
    dec f = add;
    dec result = apply(f, 10, 20);
    
    // Type mismatch error
    dec bad_f: fn(i32) -> i32 = add; // ERROR

    return result;
}
