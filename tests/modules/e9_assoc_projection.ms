trait HasAssoc {
    type Value;
    fn get_value(self: &Self) -> Self::Value;
}

struct Box {
    val: int_32;
}

impl HasAssoc for Box {
    type Value = int_32;
    fn get_value(self: &Box) -> int_32 {
        return self.val;
    }
}

fn extract<T: HasAssoc>(item: &T) -> T::Value {
    return item.get_value();
}

fn main() -> int_32 {
    dec b = Box { val: 42 };
    dec res = extract@<Box>(&b);
    if res == 42 {
        return 0;
    }
    return 1;
}
