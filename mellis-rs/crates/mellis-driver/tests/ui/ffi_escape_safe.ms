// NOTE: uses named fn pointer, not closure — closures not yet implemented, see roadmap 2.1
extern fn register_callback(ptr: *rw i32, cb: fn(*rw i32));

fn my_callback(x: *rw i32) {}

fn main() {
    dec rw x = 10;
    dec ptr: *rw i32 = &rw x;
    register_callback(ptr, my_callback);
    // Passing ptr into FFI function without @sync_noescape means it MayEscape
    // thus holding the borrow indefinitely.
    x = 20; //~ ERROR Cannot access
}
