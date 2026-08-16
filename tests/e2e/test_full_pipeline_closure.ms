fn make_adder(x: int_32) -> fn(int_32) -> int_32 {
    return |y: int_32| {
        return x + y;
    };
}

fn apply_op(f: fn(int_32) -> int_32, val: int_32) -> int_32 {
    return f(val);
}

fn main() -> int_32 {
    dec adder5 = make_adder(5);
    dec result = apply_op(adder5, 10);
    if result != 15 {
        return 1;
    }
    return 0;
}
