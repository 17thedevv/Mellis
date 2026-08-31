struct Resource {
    id: i32,
}

impl Drop for Resource {
    fn drop(self: &rw Resource) {}
}

async fn helper() -> i32 {
    return 10;
}

async fn test_drop() -> i32 {
    dec r = Resource { id: 1 };
    dec h = await helper();
    return r.id + h;
}

fn main() -> i32 {
    dec fut = test_drop();
    return 0;
}
