import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = alloc::vec_new<i32>();
    v.push(1);
    v.push(2);
    v.push(3);

    // iter_mut() takes &rw Self — exclusive borrow
    for item in v.iter_mut() {
        // item should be modifiable through the mutable iterator
    }

    // After the loop, exclusive borrow on v is released
    dec len = v.len();

    // Expected: 3
    return len as i32;
}
