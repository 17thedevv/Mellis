// test_for_in.ms
// Target 1: for-in loop over array — intrinsic path

fn sum_array() -> int_32 {
    dec arr = [1, 2, 3, 4, 5];
    dec sum: int_32 = 0;
    for (x in arr) {
        sum = sum + x;
    }
    return sum;  // expect 15
}

fn main() -> int_32 {
    return sum_array();
}
