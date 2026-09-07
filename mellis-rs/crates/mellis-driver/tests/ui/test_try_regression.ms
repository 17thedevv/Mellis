
trait Try {
    type Output;
    type Residual;
}

enum MyResult {
    Ok(i32),
    Err(i32),
}

impl Try for MyResult {
    type Output = i32;
    type Residual = i32;
}

fn do_work(fail: bool) -> MyResult {
    if fail {
        return MyResult::Err(42);
    }
    return MyResult::Ok(100);
}

fn caller(fail: bool) -> MyResult {
    dec val = do_work(fail)?;
    return MyResult::Ok(val * 2);
}

fn main() -> i32 {
    dec res1 = caller(false);
    dec res2 = caller(true);
    
    // Check res1 (expect Ok(200)) and res2 (expect Err(42))
    // We will just return 200 + 42 = 242 if everything works
    dec rw sum: i32 = 0;
    match res1 {
        MyResult::Ok(v) -> { sum = sum + v; },
        MyResult::Err(e) -> { sum = sum + 0; },
    };
    
    match res2 {
        MyResult::Ok(v) -> { sum = sum + 0; },
        MyResult::Err(e) -> { sum = sum + e; },
    };
    
    return sum;
}
