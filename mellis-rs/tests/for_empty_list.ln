import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = alloc::vec_new<i32>();
    // Empty vector - loop body should never execute

    dec rw sum: i32 = 0;
    for item in v.iter() {
        sum = sum + item;
    }

    // Expected: 0 (loop body never runs)
    return sum;
}
