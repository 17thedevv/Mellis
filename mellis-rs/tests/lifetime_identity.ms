fn identity<'a>(x: &'a i32) -> &'a i32 {
    return x;
}

fn main() {
    dec rw val = 42;
    dec x = identity(&val);
    val = 100; // should fail because val is borrowed by x
    dec y = *x; // use x
}
