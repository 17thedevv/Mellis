async fn one() -> i32 {
    return 1;
}

async fn two() -> i32 {
    return 2;
}

async fn three() -> i32 {
    return 3;
}

async fn add_three() -> i32 {
    dec a = await one();
    dec b = two().await;
    dec c = await three();
    return a + b + c;
}

fn main() -> i32 {
    dec fut = add_three();
    return 0;
}
