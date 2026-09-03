struct Vec<T> {
    data: T;
}

struct Map<K, V> {
    key: K;
    value: V;
}

fn identity<T>(x: T) -> T {
    return x;
}

fn main() {
    dec my_vec: Vec<int_32> = Vec { data: 42 };
    dec nested: Vec<Vec<int_32> > = Vec { data: my_vec };
    dec val = identity(42);
    dec b_val = identity(true);
}
