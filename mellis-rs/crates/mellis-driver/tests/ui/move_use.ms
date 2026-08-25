struct S { a: i32 }
fn consume(s: S) {}

fn main() {
    dec x = S { a: 10 };
    consume(x);
    dec y = x; //~ ERROR Use of moved value
}
