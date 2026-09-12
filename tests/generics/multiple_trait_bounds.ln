trait Printable {
    fn print(self);
}

trait Hashable {
    fn hash(self) -> int_32;
}

struct Point {
    x: int_32;
    y: int_32;
}

impl Printable for Point {
    fn print(self) {
        // ...
    }
}

impl Hashable for Point {
    fn hash(self) -> int_32 {
        return self.x + self.y;
    }
}

fn do_something<T: Printable + Hashable>(val: T) {
    val.print();
}

fn do_another<T: Printable + Hashable>(val: T) {
    dec h = val.hash();
}

fn main() -> int_32 {
    dec p = Point { x: 10, y: 20 };
    dec p2 = Point { x: 5, y: 15 };
    do_something(p);
    do_another(p2);
    return 0;
}
