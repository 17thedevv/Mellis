fn main() -> int_32 {
    dec base: int_32 = 10;
    dec adder = |x: int_32| -> int_32 { 
        x + base 
    };
    return adder(5);
}
