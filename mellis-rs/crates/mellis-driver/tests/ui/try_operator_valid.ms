struct Error { code: i32 }
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn inner() -> Result<i32, Error> {
    return Result::Ok(42);
}

fn outer() -> Result<i32, Error> {
    dec x = inner()?;
    return Result::Ok(x + 1);
}

fn main() {
    dec res = outer();
}
