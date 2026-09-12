struct Point {
    x: int_32;
    y: int_32;
}

impl Point {
    // No static methods used to keep it simple

    fn add(self: Point, other: Point) -> Point {
        return Point {
            x: self.x + other.x,
            y: self.y + other.y
        };
    }
}

struct Vec<T> {
    data: T;
}

impl<T> Vec<T> {
    fn get(self: Vec<T>) -> T {
        return self.data;
    }
}

fn main() {
    dec p1 = Point { x: 1, y: 2 };
    dec p2 = Point { x: 3, y: 4 };
    dec p3 = p1.add(p2);
    
    dec v: Vec<int_32> = Vec { data: 42 };
    dec x = v.get();
}
