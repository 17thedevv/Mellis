fn main() {
    dec rw a = 1;
    dec rw b = 2;
    dec tup = (&a, &b);
    a = 3; // Should fail because a is borrowed in tup
    dec v = tup;
}
