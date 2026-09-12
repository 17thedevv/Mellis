async fn step(x: i32) -> i32 {
    return x + 1;
}

async fn test_loop() -> i32 {
    dec rw sum = 0;
    dec rw i = 0;
    while i < 3 {
        sum = await step(sum);
        i = i + 1;
    }
    return sum;
}

fn main() -> i32 {
    dec fut = test_loop();
    return 0;
}
