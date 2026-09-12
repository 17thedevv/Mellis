struct StringData { length: int_32; }

fn consume(s: StringData) -> int_32 {
    return s.length;
}

fn main() -> int_32 {
    dec s = StringData { length: 42 };
    dec f = move | | -> int_32 { consume(s) };
    return f();
}
