async fn compute() -> i32 {
    return 42;
}

async fn runner() -> i32 {
    dec x = compute().await;
    dec y = await compute();
    return x + y;
}

fn main() -> i32 {
    dec fut = runner();
    return 0;
}
