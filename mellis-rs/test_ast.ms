
fn main() {
    dec rw a = 10;
    dec rw b = &rw a;
    unsafe { *rw b = 20; }
}
