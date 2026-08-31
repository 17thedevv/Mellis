fn takes_ref(x: &i32) -> i32 {
    return *x;
}

fn main() {
    dec a = 42;
    dec r = &a;
    dec v = takes_ref(r);
}
