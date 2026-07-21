// =============================================================================
// lib/runtime.ms
// 
// Mellis Runtime Standard Library
// =============================================================================

// Giao tiep voi OS / libc
extern fn printf(fmt: *int_8, ...) -> int_32;
extern fn mellis_coro_is_done(handle: *int_8) -> bool;
extern fn mellis_coro_resume(handle: *int_8);

export fn print(val: int_32) {
    unsafe {
        printf("%d\n" as *int_8, val);
    }
}

// Vong lap su kien bat dong bo
export fn block_on(handle: *int_8) {
    unsafe {
        while !mellis_coro_is_done(handle) {
            mellis_coro_resume(handle);
        }
    }
}
