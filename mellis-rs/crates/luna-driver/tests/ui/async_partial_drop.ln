struct Item {
    val: i32,
}

struct Foo {
    a: Item,
    b: Item,
}

fn consume_item(i: Item) -> i32 {
    return i.val;
}

async fn async_work() -> i32 {
    return 1;
}

async fn test_partial() -> i32 {
    dec foo = Foo {
        a: Item { val: 10 },
        b: Item { val: 20 },
    };
    consume_item(foo.a);
    dec res = async_work().await;
    return foo.b.val + res;
}

fn main() -> i32 {
    return 0;
}
