fn main() -> i32 {
    dec f: fn(i32, i32) -> i32 = |a: i32, b: i32| -> i32 {
        return a + b;
    };
    return f(5, 7);
}
