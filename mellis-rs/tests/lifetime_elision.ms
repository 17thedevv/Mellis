fn identity(x: &i32) -> &i32 {
    return x;
}

fn outer(x: &i32) -> &i32 {
    return identity(x);
}

fn main() {
    dec rw val = 42;
    dec x = outer(&val);
    val = 100; // should fail because val is borrowed by x
    dec y = x; // use x
}
