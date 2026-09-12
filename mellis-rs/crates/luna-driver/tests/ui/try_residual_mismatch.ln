

fn do_work() -> Result<i32, i32> {
    return Result::Err(42);
}

// Function returns Option-like (we'll just use another Result with different error type)
// String does not implement FromResidual<Result<Never, i32>>
fn caller() -> Result<i32, bool> {
    dec val = do_work()?; // Error: cannot convert residual
    return Result::Ok(val * 2);
}

fn main() -> i32 {
    return 0;
}
