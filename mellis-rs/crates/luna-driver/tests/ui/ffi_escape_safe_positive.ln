// NOTE: uses named fn pointer, not closure — closures not yet implemented, see roadmap 2.1
extern fn register_callback_safe(#[sync_noescape] ptr: *rw i32, cb: fn(*rw i32));

fn my_callback(x: *rw i32) {}

fn main() {
    dec rw x = 10;
    dec ptr = &rw x;
    register_callback_safe(ptr, my_callback);
    // Because of #[sync_noescape], the borrow does not escape.
    // So the borrow ends here.
    x = 20;
}
