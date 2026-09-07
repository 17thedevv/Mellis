struct NoDefaultType {
    val: i32,
}

#[derive(Default)]
struct Outer {
    inner: NoDefaultType,
}

fn main() -> i32 {
    return 0;
} //~ ERROR Symbol 'NoDefaultType::default' not found //~ ERROR Type mismatch for field 'inner'
