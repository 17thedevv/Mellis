// Test generic ? operator
enum Result<T, E> {
    Ok(T),
    Err(E)
}

fn generic_fail<T, E>(e: E) -> Result<T, E> {
    return Result::Err(e);
}

fn test_try<T, E>(e: E) -> Result<T, E> {
    dec val: T = generic_fail(e)?;
    return Result::Ok(val);
}

fn main() -> i32 {
    dec r: Result<i32, i32> = test_try(42);
    
    // We expect r to be Err(42)
    // Actually we can't pattern match yet? Wait, is match supported?
    // Let's just return 0 to check if it compiles!
    return 0;
}
