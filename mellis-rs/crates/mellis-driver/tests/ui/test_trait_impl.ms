trait Animal {
    fn speak(self) -> i32;
}

struct Dog {}

impl Animal for Dog {
    fn speak(self) -> i32 {
        return 1;
    }
}

fn main() {
    dec d = Dog {};
    dec s = d.speak();
}
