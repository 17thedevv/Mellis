// Negative test: move ownership tracking in comptime
// Verifies PlaceState accurately triggers UseOfUninitializedOrMoved

struct MyStruct {
    a: i32,
    b: i32,
}

fn consume(s: MyStruct) -> i32 {
    s.a + s.b
}

const OWNERSHIP_VIOLATION: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    dec s = MyStruct { a: 10, b: 20 };
    consume(s);
    consume(s) // Error: s has been moved
};

fn main() -> i32 {
    OWNERSHIP_VIOLATION
}
