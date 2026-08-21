fn main() {
    dec x: int_32 = 10;
    dec p = &x as *rw int_32;
    dec y = p.add(1); // Error: requires unsafe

    dec p2 = p + 2;   // Error: requires unsafe
}
