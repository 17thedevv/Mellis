enum Result<T, E> {
    Ok(T),
    Err(E),
}

async fn fetch_number(success: bool) -> Result<i32, i32> {
    if success {
        return Result::Ok(42);
    } else {
        return Result::Err(1);
    }
}

async fn try_runner() -> Result<i32, i32> {
    dec val = fetch_number(true).await?;
    return Result::Ok(val + 10);
}

fn main() -> i32 {
    return 0;
}
