trait Display {
    fn fmt(self: &Self);
}

struct Point {
    x: int_32;
    y: int_32;
}

impl Display for Point {
    fn fmt(self: &Point) {
        // ...
    }
}

fn print_it<T: Display>(item: &T) {
    item.fmt();
}

fn main() {
    dec p = Point { x: 10, y: 20 };
    print_it(&p);
}
