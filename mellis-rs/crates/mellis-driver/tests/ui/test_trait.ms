trait Foo {
    fn bar(self: Foo) -> i32;
}

struct MyStruct { x: i32; }

impl Foo for MyStruct {
    fn bar(self: MyStruct) -> i32 {
        return self.x;
    }
}

fn main() {
    dec s = MyStruct { x: 42 };
    dec y = s.bar();
}
