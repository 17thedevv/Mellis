fn foo(a: *rw i32) {}

fn main() {
    dec rw x = 10;
    dec p1 = &rw x;
    foo(p1); // p1's last use is here, lifetime ends
    
    dec p2 = &rw x; // OK because of NLL
    foo(p2);
}
