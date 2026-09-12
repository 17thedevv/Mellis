macro make_container {
    (@name: ident) => {
        struct @name<T> {
            val: T,
        }
    }
}

make_container!(MyContainer);

fn main() -> i32 {
    dec c: MyContainer<i32> = MyContainer { val: 42 };
    return c.val;
}
