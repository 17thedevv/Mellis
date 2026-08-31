fn split(x: &i32) -> (&i32, &i32) {
    return (x, x);
}
fn main() {
    dec rw a = 1;
    dec t = split(&a);
    a = 2; // Should fail because a is borrowed in t
    dec v = t;
}
