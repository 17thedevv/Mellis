async fn helper() -> i32 {
    return 10;
}

async fn test_conflict() -> i32 {
    dec rw x = 10;
    dec r = &x;
    await helper();
    dec r2 = &rw x; //~ ERROR Cannot borrow
    return *r + *r2;
}

fn main() -> i32 {
    return 0;
}
