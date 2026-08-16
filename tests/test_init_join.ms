fn main() {
    dec x: int_32;
    dec cond = true;
    if cond {
        x = 10;
    } else {
        // x is uninitialized here
    }
    
    // ERROR: x might be uninitialized here because of the 'else' branch
    dec y = x;
}
