struct Pair {
    first: i32,
    second: i32,
}

async fn helper() -> i32 {
    return 10;
}

async fn test_partial() -> i32 {
    dec p = Pair { first: 1, second: 2 };
    dec x = p.first;
    dec h = await helper();
    return x + p.second + h;
}

fn main() -> i32 {
    dec fut = test_partial();
    return 0;
}
