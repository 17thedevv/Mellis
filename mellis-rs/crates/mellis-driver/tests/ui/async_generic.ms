async fn identity<T>(x: T) -> T {
    return x;
}

async fn run_generic() -> i32 {
    dec a = await identity<i32>(42);
    return a;
}

fn main() -> i32 {
    dec fut = run_generic();
    return 0;
}
