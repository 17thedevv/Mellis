fn main() {
    dec x = 10;
    dec y = 20;
    
    dec cond = true;
    dec r: &rw int_32;
    if cond {
        r = &rw x;
    } else {
        r = &rw y;
    }
    
    // r points to either x or y.
    // We should not be able to mutate x or y directly here, since they might be borrowed mutably by r.
    x = 30; // ERROR: x is potentially borrowed
    *r = 40;
    
    dec z = y; // ERROR: y is potentially borrowed mutably
}
