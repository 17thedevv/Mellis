fn b() {}
module A {
    fn internal_f() {}
    export fn public_f() {}
}

fn main() {
    A::public_f(); // Should be valid
    A::b(); // Should be invalid (b is not in A!)
    A::internal_f(); // Should be invalid (internal_f is not exported!)
}
