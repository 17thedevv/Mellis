fn main() {
    dec x: int_32 = 10; // int_32
    dec y: int_64 = 20; // int_64

    unsafe {
        dec px = &x as *rw int_32;
        dec py = &y as *rw int_64;

        dec diff = px - py; // Error: same pointee type required
    }
}
