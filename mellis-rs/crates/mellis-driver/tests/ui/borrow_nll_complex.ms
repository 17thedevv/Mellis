fn identity(x: &rw i32) -> &rw i32 {
    return x;
}

fn test_loop_borrows() {
    dec rw a = 10;
    while true {
        dec rw b = &rw a;
        *b = 20;
    } // b dies here, so a can be borrowed again in next iteration
}

fn test_conditional_return(cond: bool, a: &rw i32, b: &rw i32) -> &rw i32 {
    if cond {
        return a;
    }
    return b;
}

fn test_branch_join() {
    dec rw x = 1;
    dec rw y = 2;
    dec rw z = &rw x;
    if true {
        z = &rw y;
    }
    // z could be pointing to x or y here.
    // So both x and y are borrowed!
    x = 3; //~ ERROR Cannot access '%v0'
    y = 4; //~ ERROR Cannot access '%v2'
    *z = 5;
}
