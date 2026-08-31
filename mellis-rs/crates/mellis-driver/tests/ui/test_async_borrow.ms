async fn g() {}

async fn f() {
    dec rw x = 10;
    dec r = &rw x;
    await g();
    use(r);
}

fn use(r: &rw i32) {}

fn main() {}
