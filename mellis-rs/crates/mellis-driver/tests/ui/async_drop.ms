struct Resource {
    id: i32,
}

async fn worker() -> i32 {
    return 100;
}

async fn resource_lifecycle() -> i32 {
    dec res = Resource { id: 1 };
    dec val = worker().await;
    return res.id + val;
}

fn main() -> i32 {
    dec fut = resource_lifecycle();
    return 0;
}
