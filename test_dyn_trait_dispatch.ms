trait Drawable {
    fn draw(self: &Self);
    fn area(self: &Self) -> int_32;
}

struct Circle {
    radius: int_32;
}

impl Drawable for Circle {
    fn draw(self: &Circle) {
    }
    fn area(self: &Circle) -> int_32 {
        return self.radius * self.radius * 3;
    }
}

struct Square {
    side: int_32;
}

impl Drawable for Square {
    fn draw(self: &Square) {
    }
    fn area(self: &Square) -> int_32 {
        return self.side * self.side;
    }
}

fn main() -> int_32 {
    dec c: Circle;
    c.radius = 10;
    
    dec s: Square;
    s.side = 5;

    dec dyn_c = &c as &dyn Drawable;
    dec dyn_s = &s as &dyn Drawable;

    dec area_c: int_32 = dyn_c.area();
    dec area_s: int_32 = dyn_s.area();

    if (area_c != 300) { return area_c; }
    if (area_s != 25) { return area_s; }

    return 0;
}
