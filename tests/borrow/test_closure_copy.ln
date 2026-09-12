fn main() -> int_32 {
    dec x = 10;
    dec f = | | -> int_32 { x };
    x = 20; // mutate outer
    return f(); // MUST return 10 (captured value)
}
