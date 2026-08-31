struct Data {
    val: i32,
}

async fn helper() -> i32 {
    return 10;
}

async fn test_closure_move() -> i32 {
    dec d = Data { val: 50 };
    dec take_d = move |extra: i32| -> i32 { return d.val + extra; };
    dec x = await helper();
    return take_d(0) + x;
}

fn main() -> i32 {
    dec fut = test_closure_move();
    return 0;
}
