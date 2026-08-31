fn identity<'a>(x: &'a i32) -> &'a i32 {
    return x;
}

fn main() {
    dec val = 42;
    dec x = identity(&val);
}
