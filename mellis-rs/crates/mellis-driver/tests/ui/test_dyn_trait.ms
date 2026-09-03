trait Foo {
    fn bar(self: Self) -> i32;
}

struct MyStruct { x: i32; }

impl Foo for MyStruct {
    fn bar(self: MyStruct) -> i32 {
        return self.x;
    }
}

fn call_foo(obj: dyn Foo) -> i32 {
    return obj.bar();
}

fn main() {
    dec s = MyStruct { x: 42 };
    call_foo(s);
}
