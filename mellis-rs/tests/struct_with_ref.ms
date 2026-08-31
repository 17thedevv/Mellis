struct RefWrapper {
    ptr: &i32,
}
fn main() {
    dec rw a = 1;
    dec wrapper = RefWrapper { ptr: &a };
    a = 2; // Should fail because a is borrowed by wrapper
    dec v = wrapper.ptr;
}
