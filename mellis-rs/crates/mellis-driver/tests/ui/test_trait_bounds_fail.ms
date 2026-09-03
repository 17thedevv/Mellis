trait Printable {
    fn print(self);
}

struct Unprintable {}

fn print_it<T: Printable>(val: T) {
    val.print();
}

fn main() {
    dec u = Unprintable {};
    print_it<Unprintable>(u); //~ ERROR does not implement trait `Printable`
}
