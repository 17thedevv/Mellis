
import "mock";

fn attempt_operation() -> MyResult<i32, i32> {
    return MyResult::MyOk(42);
}

fn do_work() -> MyResult<i32, i32> {
    dec val = attempt_operation()?;
    return MyResult::MyOk(val * 2);
}

fn generic_try<T, E>(x: MyResult<T, E>) -> MyResult<T, E> {
    dec val = x?;
    return MyResult::MyOk(val);
}

fn main() -> i32 {
    return 0;
}
