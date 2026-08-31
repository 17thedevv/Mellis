struct StructA { val: i32 }
struct StructB { val: i32 }

trait Drop {
    fn drop(self: &rw Self);
}

impl Drop for StructA {
    fn drop(self: &rw Self) {}
}

impl Drop for StructB {
    fn drop(self: &rw Self) {}
}

fn do_something_a(a: StructA) {}
fn do_something_b(b: StructB) {}

fn main() {
    dec a = StructA { val: 1 };
    dec b = StructB { val: 2 };
    do_something_a(a);
    do_something_b(b);
}
