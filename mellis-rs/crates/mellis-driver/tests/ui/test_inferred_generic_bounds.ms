trait Animal {
    fn speak(self) -> i32;
}

struct Dog {}

impl Animal for Dog {
    fn speak(self) -> i32 {
        return 1;
    }
}

struct Cat {}

fn make_sound<T: Animal>(animal: &T) -> i32 {
    return animal.speak();
}

fn main() -> i32 {
    dec dog = Dog {};
    dec dog_sound = make_sound(&dog); // OK: Dog implements Animal
    
    dec cat = Cat {};
    dec cat_sound = make_sound(&cat); //~ ERROR The type `Cat` does not implement trait `Animal` (required by inferred generic parameter `T`)

    return dog_sound;
}
