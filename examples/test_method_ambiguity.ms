struct Point { x: int_32; y: int_32; }

impl Point {
    fn test(self: &Point) -> int_32 { return 1; }
}

impl Point {
    fn test(self: &Point) -> int_32 { return 2; }
}

fn main() {
    dec p = Point { x: 1, y: 2 };
    
    // Explicit borrow needed!
    dec r = &p;
    r.test(); // Should error: Ambiguous method call 'test'
}
