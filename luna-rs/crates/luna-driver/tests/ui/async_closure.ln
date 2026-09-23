async fn helper() -> i32 {
    return 10;
}

async fn test_closure() -> i32 {
    dec add = |a: i32, b: i32| -> i32 { return a + b; };
    dec x = await helper();
    return add(x, 5);
}

fn main() -> i32 {
    dec fut = test_closure();
    return 0;
}
