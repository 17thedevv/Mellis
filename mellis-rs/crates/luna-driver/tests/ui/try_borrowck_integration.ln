
trait Try {
    type Output;
    type Residual;
}

enum Result {
    Ok(i32),
    Err(i32),
}

impl Try for Result {
    type Output = i32;
    type Residual = i32;
}

struct Resource {
    id: i32,
}

impl Drop for Resource {
    fn drop(self: &rw Self) {
        // drop Resource
    }
}

fn test_success_try() -> Result {
    dec rw r = Resource { id: 1 };
    dec val = Result::Ok(42);
    dec unwrapped = val?;
    return Result::Ok(unwrapped);
}

fn test_fail_try() -> Result {
    dec rw r = Resource { id: 2 };
    dec val = Result::Err(99);
    dec unwrapped = val?;
    return Result::Ok(unwrapped);
}

fn test_move_try() -> Result {
    dec rw r = Resource { id: 3 };
    dec val = Result::Err(99);
    dec moved_r = r; // moved here
    dec unwrapped = val?; // fail here, should not drop r again
    return Result::Ok(unwrapped);
}

fn main() -> i32 {
    test_success_try();
    test_fail_try();
    test_move_try();
    return 0;
}
