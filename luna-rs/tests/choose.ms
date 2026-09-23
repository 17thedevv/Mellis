fn choose(a: &i32, b: &i32, cond: bool) -> &i32 {
    if cond { return a; }
    return b;
}
fn main() {
    dec x = 1;
    dec rw y = 2;
    dec z = choose(&x, &y, true);
    y = 3; // Should fail because y is borrowed via z
    dec w = z;
}
