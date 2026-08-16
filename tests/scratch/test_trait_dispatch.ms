trait Drawable {
    fn draw(self);
}

struct Circle {
    r: int_32;
}

impl Drawable for Circle {
    fn draw(self) {
        // ...
    }
}

// Object safety violation (generic method)
trait InvalidTrait {
    fn do_something<T>(self, val: T);
}

impl InvalidTrait for Circle {
    fn do_something<T>(self, val: T) {
        // ...
    }
}

fn static_dispatch(c: Circle) {
    c.draw();
}

fn dyn_dispatch(d: &dyn Drawable) {
    d.draw();
}

fn main() -> int_32 {
    dec c: Circle;
    c.r = 5;
    static_dispatch(c);
    
    // Coercion from &Circle to &dyn Drawable
    dec d: &dyn Drawable = &c;
    dyn_dispatch(d);
    
    // Error expected:
    dec bad: &dyn InvalidTrait = &c;
    
    return 0;
}
