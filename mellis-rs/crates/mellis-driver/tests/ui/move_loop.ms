struct S { a: i32 }
fn consume(s: S) {}

fn main() {
    dec rw x = S { a: 10 };
    while true {
        consume(x); //~ ERROR Use of moved value
    }
}
