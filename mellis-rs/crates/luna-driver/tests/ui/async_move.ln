struct Resource {
    val: i32,
}

fn consume(r: Resource) -> i32 {
    return r.val;
}

async fn do_async() -> i32 {
    return 10;
}

async fn test_move() -> i32 {
    dec r = Resource { val: 100 };
    consume(r);
    dec res = do_async().await;
    return r.val; //~ ERROR Use of moved value
}

fn main() -> i32 {
    return 0;
}
