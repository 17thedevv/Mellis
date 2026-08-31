fn identity<T>(x: T) -> T {
    return x;
}

fn main() {
    dec a = identity<i32>(42);
    dec b = identity<bool>(true);
}
