intrinsic fn ptr_offset<T>(p: *const T, offset: int_32) -> *const T;

fn main() {
    dec arr = [1, 2, 3];
    dec p = &rw arr[0] as *rw int_32;
    unsafe {
        dec p2 = p + 1;
        dec p3 = p2 - 1;
        dec diff = p2 - p;
    }
}
