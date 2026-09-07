

enum CustomFlow<T, E> {
    Value(T),
    Failure(E),
}

// The residual type of CustomFlow
enum CustomResidual<E> {
    Failed(E),
}

impl<T, E> FromResidual<CustomResidual<E> > for CustomFlow<T, E> {
    fn from_residual(residual: CustomResidual<E>) -> Self {
        return match residual {
            CustomResidual::Failed(e) -> CustomFlow::Failure(e),
        };
    }
}

impl<T, E> Try for CustomFlow<T, E> {
    type Output = T;
    type Residual = CustomResidual<E>;
    
    fn from_output(v: T) -> Self {
        return CustomFlow::Value(v);
    }
    
    fn branch(self: Self) -> ControlFlow<CustomResidual<E>, T> {
        return match self {
            CustomFlow::Value(v) -> ControlFlow::Continue(v),
            CustomFlow::Failure(e) -> ControlFlow::Break(CustomResidual::Failed(e)),
        };
    }
}

// We want Result to be able to be created from CustomResidual
impl<T, E> FromResidual<CustomResidual<E> > for Result<T, E> {
    fn from_residual(residual: CustomResidual<E>) -> Self {
        return match residual {
            CustomResidual::Failed(e) -> Result::Err(e),
        };
    }
}

fn source_work(fail: bool) -> CustomFlow<i32, i32> {
    if fail {
        return CustomFlow::Failure(42);
    }
    return CustomFlow::Value(100);
}

// Function returns Result but uses `?` on CustomFlow
fn caller(fail: bool) -> Result<i32, i32> {
    dec val = source_work(fail)?;
    return Result::Ok(val * 2);
}

fn main() -> i32 {
    dec res1 = caller(false); // Expect Ok(200)
    dec res2 = caller(true);  // Expect Err(42)
    
    dec rw sum: i32 = 0;
    match res1 {
        Result::Ok(v) -> { sum = sum + v; },
        Result::Err(e) -> { sum = sum + 0; },
    };
    
    match res2 {
        Result::Ok(v) -> { sum = sum + 0; },
        Result::Err(e) -> { sum = sum + e; },
    };
    
    return sum; // Expect 242
}
