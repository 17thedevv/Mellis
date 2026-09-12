// test_dyn_dispatch.ms
// End-to-end test for dyn Trait dynamic dispatch

trait Greeter {
    fn greet(&self) -> int_32;
}

struct Dog {
    age: int_32;
}

struct Cat {
    lives: int_32;
}

impl Greeter for Dog {
    fn greet(&self) -> int_32 {
        return self.age;
    }
}

impl Greeter for Cat {
    fn greet(&self) -> int_32 {
        return self.lives;
    }
}

fn call_greet(g: &dyn Greeter) -> int_32 {
    return g.greet();
}

fn main() -> int_32 {
    dec d = Dog { age: 3 };
    dec c = Cat { lives: 9 };

    dec dg: &dyn Greeter = &d;
    dec cg: &dyn Greeter = &c;

    dec dog_val = call_greet(dg);
    dec cat_val = call_greet(cg);

    // dog_val = 3, cat_val = 9, sum = 12
    return dog_val + cat_val;
}
