import "mock";
import <core>;

fn test_drift() -> core::ControlFlow<i32, i32> {
    return core::ControlFlow::Continue(42);
}

fn main() -> i32 {
    return 0;
}
