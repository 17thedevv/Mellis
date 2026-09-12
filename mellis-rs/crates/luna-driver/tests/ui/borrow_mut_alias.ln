fn foo(a: *rw i32, b: *rw i32) {}

fn main() {
    dec rw x = 10;
    dec p1 = &rw x;
    dec p2 = &rw x; //~ ERROR Cannot borrow
    foo(p1, p2);
}
