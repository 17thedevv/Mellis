trait Container {
    type Item;
    fn get(self: &Self) -> Self::Item;
}

struct Box<T> {
    val: T;
}

impl<T> Container for Box<T> {
    type Item = T;
    fn get(self: &Self) -> T {
        return self.val;
    }
}

fn test_assoc<T: Container>(c: &T) -> T::Item {
    return c.get();
}

fn main() -> int_32 {
    dec b = Box@<int_32> { val: 42 };
    dec x = test_assoc@<Box<int_32>>(&b);
    dec y = b.get();
    return x;
}
