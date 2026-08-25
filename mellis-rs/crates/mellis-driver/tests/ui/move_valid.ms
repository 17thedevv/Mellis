struct S { a: i32 }
fn consume(s: S) {}

fn main() {
    dec rw x = S { a: 10 };
    consume(x);
    x = S { a: 20 };
    consume(x);
}
