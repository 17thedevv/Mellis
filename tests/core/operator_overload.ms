trait Add<RHS> {
    fn add(self, rhs: RHS) -> Self;
}

struct Point {
    x: int_32;
    y: int_32;
}

impl Add<Point> for Point {
    fn add(self, rhs: Point) -> Point {
        return Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y
        };
    }
}

fn main() -> int_32 {
    dec p1 = Point { x: 10, y: 20 };
    dec p2 = Point { x: 5, y: 15 };
    dec p3 = p1 + p2;
    return 0;
}
