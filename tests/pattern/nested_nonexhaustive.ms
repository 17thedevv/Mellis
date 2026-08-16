enum Option<T> {
    Some(v: T),
    None,
}

enum Result<T, E> {
    Ok(v: T),
    Err(e: E),
}

fn test_deep_missing_witness(res: Result<Option<int_32>, int_32>) -> int_32 {
    match res {
        Result::Ok(Option::Some(v)) -> v,
        Result::Err(e) -> -1,
        // Error: non-exhaustive patterns: `Result::Ok(Option::None)` not covered
    }
}

fn main() -> int_32 {
    0
}
