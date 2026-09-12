async fn helper() -> i32 {
    return 10;
}

async fn test_closure_borrow() -> i32 {
    dec captured = 20;
    dec get_cap = |dummy: i32| -> i32 { return captured + dummy; };
    dec x = await helper();
    return get_cap(0) + x;
}

fn main() -> i32 {
    dec fut = test_closure_borrow();
    return 0;
}
