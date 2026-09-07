

enum MyFlow<T, E> {
    Continue(T),
    Break(E),
}

// Residual is MyFlow<Never, E>
impl<T, E> FromResidual<MyFlow<Never, E> > for MyFlow<T, E> {
    fn from_residual(residual: MyFlow<Never, E>) -> Self {
        return match residual {
            MyFlow::Break(e) -> MyFlow::Break(e),
        };
    }
}

impl<T, E> Try for MyFlow<T, E> {
    type Output = T;
    type Residual = MyFlow<Never, E>;
    
    fn from_output(v: T) -> Self {
        return MyFlow::Continue(v);
    }
    
    fn branch(self: Self) -> ControlFlow<MyFlow<Never, E>, T> {
        return match self {
            MyFlow::Continue(v) -> ControlFlow::Continue(v),
            MyFlow::Break(e) -> ControlFlow::Break(MyFlow::Break(e)),
        };
    }
}

fn do_work(fail: bool) -> MyFlow<i32, i32> {
    if fail {
        return MyFlow::Break(42);
    }
    return MyFlow::Continue(100);
}

fn caller(fail: bool) -> MyFlow<i32, i32> {
    dec val = do_work(fail)?;
    return MyFlow::Continue(val * 2);
}

fn main() -> i32 {
    dec res1 = caller(false);
    dec res2 = caller(true);
    
    dec rw sum: i32 = 0;
    match res1 {
        MyFlow::Continue(v) -> { sum = sum + v; },
        MyFlow::Break(e) -> { sum = sum + 0; },
    };
    
    match res2 {
        MyFlow::Continue(v) -> { sum = sum + 0; },
        MyFlow::Break(e) -> { sum = sum + e; },
    };
    
    return sum; // Expect 200 + 42 = 242
}
