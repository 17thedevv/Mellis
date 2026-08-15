// test_tuple_index.ms
// Target 1: Tuple indexing — exercises t.0, t.1, nested t.0.1

fn tuple_basic() -> int_32 {
    dec t = (10, 20);
    dec x = t.0;
    dec y = t.1;
    return x + y;  // expect 30
}

fn tuple_nested() -> int_32 {
    dec t = ((1, 2), 3);
    dec inner = t.0;
    dec val = inner.1;
    return val;  // expect 2
}

fn main() -> int_32 {
    dec a = tuple_basic();
    dec b = tuple_nested();
    return a + b;  // expect 32
}
