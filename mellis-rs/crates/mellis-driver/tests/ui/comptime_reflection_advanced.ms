// Positive test: reflection and sizeof capabilities at comptime

struct ComplexStruct<T> {
    field1: T,
    field2: i64,
    field3: bool,
}

const SIZE_I32: usize = comptime {
    sizeof(i32)
};

const SIZE_COMPLEX: usize = comptime {
    sizeof(ComplexStruct<i32>)
};

fn main() -> usize {
    SIZE_I32 + SIZE_COMPLEX
}
