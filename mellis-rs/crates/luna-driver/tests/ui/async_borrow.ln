struct Counter {
    val: i32,
}

fn read_counter(c: &Counter) -> i32 {
    return c.val;
}

async fn get_step() -> i32 {
    return 5;
}

async fn test_borrow() -> i32 {
    dec x = Counter { val: 10 };
    dec r = &x;
    dec step = get_step().await;
    return read_counter(r) + step;
}

fn main() -> i32 {
    return 0;
}
