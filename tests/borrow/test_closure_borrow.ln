struct Data { val: int_32; }

fn main() -> int_32 {
    dec d = Data { val: 10 };
    dec f = | | -> int_32 { d.val };
    d.val = 20; // mutate outer
    return f(); // MUST return 20 (captured by reference)
}
