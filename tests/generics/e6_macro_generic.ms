struct Box<T> {
    value: T;
}

macro create_box(@val: expr) {
    Box@<int_32> { value: @val }
}

fn main() -> int_32 {
    dec b = create_box!(42);
    return b.value;
}
