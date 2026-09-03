trait Clone {
    fn clone(self: &rw Self) -> Self;
}

struct Point {
    x: i32,
    y: i32,
}

impl Clone for Point {
    fn clone(self: &rw Point) -> Point {
        return Point { x: self.x, y: self.y };
    }
}

struct Unclonable {
    z: i32,
}

struct Node<T> {}

impl<T> Node<T> {
    fn duplicate<U: Clone>(self: &rw Node<T>, item: &rw U) -> U {
        return item.clone();
    }
}

fn main() {
    dec rw p = Point { x: 1, y: 2 };
    dec rw u = Unclonable { z: 3 };
    dec rw node = Node<i32> {};

    dec p2 = node.duplicate(&rw p); // OK
    dec u2 = node.duplicate(&rw u); //~ ERROR The type `Unclonable` does not implement trait `Clone` (required by inferred generic parameter `U`)
}
