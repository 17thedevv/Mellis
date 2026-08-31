struct Data {
    val: i32,
}

async fn helper() -> i32 {
    return 10;
}

fn consume(d: Data) -> i32 {
    return d.val;
}

async fn test_move() -> i32 {
    dec d = Data { val: 42 };
    consume(d);
    await helper();
    return d.val; //~ ERROR Use of moved value
}

fn main() -> i32 {
    return 0;
}
