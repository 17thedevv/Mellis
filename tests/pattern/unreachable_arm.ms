fn test_unreachable_arm(x: int_32) -> int_32 {
    match x {
        _ -> 0,
        1 -> 1, // Warning: unreachable
    }
}

fn main() -> int_32 {
    0
}
