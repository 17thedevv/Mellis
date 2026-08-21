unsafe fn dangerous_op() -> int_32 {
    return 42;
}

fn safe_caller() -> int_32 {
    dec x = 0;
    
    // Valid unsafe block
    unsafe {
        x = dangerous_op();
    }
    
    return x;
}

fn main() {
    dec p = safe_caller();
    
    // Testing Function Pointer Loophole
    dec ptr: unsafe fn() -> int_32 = dangerous_op;
    
    // Calling an unsafe function via a pointer should require unsafe
    unsafe {
        dec y = ptr();
    }
}
