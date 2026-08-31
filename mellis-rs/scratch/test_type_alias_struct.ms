struct MyStruct<T> {
    val: T,
}

type MyStructAlias<K> = MyStruct<K>;

fn main() -> i32 {
    dec s: MyStructAlias<i32> = MyStructAlias<i32> { val: 42 };
    return s.val;
}
