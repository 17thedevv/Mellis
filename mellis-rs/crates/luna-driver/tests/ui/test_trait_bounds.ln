trait Printable {
    fn print(self);
}

struct MyStruct {}

impl Printable for MyStruct {
    fn print(self) {
    }
}

struct Unprintable {}

fn print_it<T: Printable>(val: T) {
    val.print();
}

fn main() {
    dec m = MyStruct {};
    print_it<MyStruct>(m); // This should succeed
}
