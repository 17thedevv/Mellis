trait Calculator {
    fn add(self: &Self, x: i32) -> i32;
    fn get_base(self: &Self) -> i32;
}

struct SimpleCalc {
    base: i32,
}

impl Calculator for SimpleCalc {
    fn add(self: &SimpleCalc, x: i32) -> i32 {
        return self.base + x;
    }
    fn get_base(self: &SimpleCalc) -> i32 {
        return self.base;
    }
}

fn execute_calc(c: &dyn Calculator) -> i32 {
    dec a = c.add(10);
    dec b = c.get_base();
    return a + b;
}

fn main() {
    dec sc = SimpleCalc { base: 5 };
    dec r = execute_calc(&sc);
}
