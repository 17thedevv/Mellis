async fn helper() -> i32 {
    return 100;
}

async fn test_borrow() -> i32 {
    dec x = 42;
    dec r = &x;
    dec h = await helper();
    return *r + h;
}

fn main() -> i32 {
    dec fut = test_borrow();
    return 0;
}
