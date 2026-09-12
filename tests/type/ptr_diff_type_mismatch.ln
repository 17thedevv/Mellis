fn main() {
    dec x = 10;
    dec y = 20;
    dec p1 = &rw x as *rw int_32;
    dec p2 = &rw y as *rw int_64; // Different type
    unsafe {
        dec diff = p1 - p2; // Should error
    }
}
