trait Drawable {
    fn draw(self: &Self);
}

trait Debug {
    fn debug(self: &Self);
}

struct Circle {
    radius: float_64;
}

// 1. Missing method
impl Drawable for Circle {
}

// 2. Duplicate Impl
impl Drawable for Circle {
    fn draw(self: &Circle) {
    }
}

// 3. Signature mismatch
struct Square {
    side: float_64;
}

impl Drawable for Square {
    fn draw(self: &Square, extra: int_32) {
    }
}

// 4. Ambiguous method
trait TraitA { fn foo(self: &Self); }
trait TraitB { fn foo(self: &Self); }

impl TraitA for int_32 { fn foo(self: &int_32) {} }
impl TraitB for int_32 { fn foo(self: &int_32) {} }

fn main() {
    dec x = 0;
    x.foo();
}
