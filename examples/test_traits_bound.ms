trait Display {
    fn fmt(self: &Self);
}

struct Point {
    x: int_32;
    y: int_32;
}

impl Display for Point {
    fn fmt(self: &Point) {}
}

struct Line {
    p1: Point;
    p2: Point;
}

fn print_it<T: Display>(item: &T) {
}

fn main() {
    dec p = Point { x: 10, y: 20 };
    print_it(&p);
    
    dec l = Line { p1: p, p2: p };
    print_it(&l);
}
