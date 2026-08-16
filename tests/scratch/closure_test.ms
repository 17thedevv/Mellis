fn main() -> int_32 {
    dec x = 5;
    dec add5 = |y: int_32| -> int_32 {
        return x + y;
    };
    return add5(10) - 15; // Should be 0
}
