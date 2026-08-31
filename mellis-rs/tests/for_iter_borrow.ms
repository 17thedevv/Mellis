import <core>;
import <alloc>;

fn sum_vec(v: &alloc::Vec<i32>) -> i32 {
    dec rw total: i32 = 0;
    for item in v.iter() {
        total = total + item;
    }
    return total;
}

fn main() -> i32 {
    dec rw v = alloc::vec_new<i32>();
    v.push(1);
    v.push(2);
    v.push(3);

    // Pass by shared borrow — should be fine
    dec s = sum_vec(&v);

    // Vec is still usable after shared borrow ends
    dec len = v.len();

    // Expected: 6
    return s;
}
