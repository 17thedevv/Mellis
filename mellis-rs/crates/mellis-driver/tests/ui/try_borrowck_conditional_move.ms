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
    fn drop(self: &rw Self) { }
}

fn test_conditional_move() -> Result {
    dec rw r = Resource { id: 1 }; //~ ERROR Cannot drop conditionally moved value
    dec val = Result::Err(99);
    
    if true {
        dec moved_r = r;
    }
    
    dec unwrapped = val?;
    
    return Result::Ok(unwrapped);
}

fn main() -> i32 {
    return 0;
}
