trait Display {
    fn fmt(self: &Self);
}

struct Point {
    x: i32,
    y: i32,
}

impl Display for Point {
    fn fmt(self: &Self) {
        // ...
    }
}

fn print_it<T: Display>(item: &T) {
    item.fmt();
}

fn main() {
    let p = Point { x: 10, y: 20 };
    print_it(&p);
}
