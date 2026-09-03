
fn main() -> i32 {
    dec x = 10;
    dec f = || -> i32 {
        return x + 1;
    };
    return f();
}
