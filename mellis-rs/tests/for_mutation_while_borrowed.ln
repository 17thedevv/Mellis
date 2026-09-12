import <core>;
import <alloc>;

fn main() -> i32 {
    dec rw v = alloc::vec_new<i32>();
    v.push(1);
    v.push(2);
    v.push(3);

    // iter() borrows v as &
    for item in v.iter() {
        // ILLEGAL: v is borrowed as & by iter(),
        // cannot mutate v while iterating
        v.push(4);
    }

    return 0;
}
