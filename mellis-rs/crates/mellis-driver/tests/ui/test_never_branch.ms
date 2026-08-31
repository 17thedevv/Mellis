fn panic() -> ! {
    while true {}
}

fn foo(b: bool) -> i32 {
    if b {
        return 10;
    } else {
        panic();
    }
}

fn main() {
    foo(false);
}
