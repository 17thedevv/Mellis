trait Foo { fn m(self: *Self); }
trait Bar { fn m(self: *Self); }

struct MyStruct {}

impl Foo for MyStruct { fn m(self: *Self) {} }
impl Bar for MyStruct { fn m(self: *Self) {} }

fn test(x: *MyStruct) {
    x.m(); // Error: E-TRAIT-AMBIGUOUS
}

fn main() -> int_32 { 0 }
