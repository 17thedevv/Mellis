async fn helper() -> i32 {
    return 10;
}

async fn test_local() -> i32 {
    dec a = 5;
    dec b = 15;
    dec x = await helper();
    dec c = 20;
    return a + b + x + c;
}

fn main() -> i32 {
    dec fut = test_local();
    return 0;
}
