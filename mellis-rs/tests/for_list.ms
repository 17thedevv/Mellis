import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = alloc::vec_new<i32>();
    v.push(10);
    v.push(20);
    v.push(30);

    dec rw sum: i32 = 0;
    for item in v.iter() {
        sum = sum + item;
    }

    // Expected: 10 + 20 + 30 = 60
    return sum;
}
