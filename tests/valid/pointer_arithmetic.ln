fn main() -> int_32 {
    dec arr: [int_32; 3] = [10, 20, 30];
    dec ptr = &arr as *rw int_32;
    unsafe {
        dec ptr2 = ptr + 2;
        dec diff = ptr2 - ptr;
        return diff as int_32;
    }
}
