trait Printable {
    fn print(self);
}
struct Dummy {}
impl Printable for Dummy {
    fn print(self) {}
}

fn foo<T: Printable>(x: T) {
    x.print();
}

fn main() {
    dec d = Dummy {};
    foo<Dummy>(d);
}
