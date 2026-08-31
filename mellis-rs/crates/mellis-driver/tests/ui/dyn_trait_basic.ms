trait Speaker {
    fn speak(self: &Self) -> i32;
}

struct Dog {
    age: i32,
}

impl Speaker for Dog {
    fn speak(self: &Dog) -> i32 {
        return self.age;
    }
}

fn call_speaker(s: &dyn Speaker) -> i32 {
    return s.speak();
}

fn main() {
    dec d = Dog { age: 7 };
    dec res = call_speaker(&d);
}
