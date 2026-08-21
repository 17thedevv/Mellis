enum Option<T> {
    Some(v: T),
    None,
}

fn test_refutable_local(opt: Option<int_32>) {
    dec Option::Some(x) = opt; // Error: refutable pattern
}

fn main() -> int_32 {
    0
}
