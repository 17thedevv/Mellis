export struct Container<T> {
    value: T,
}

export fn create_container(val: i32) -> Container<i32> {
    return Container<i32> { value: val };
}

export fn do_math(a: i32, b: i32) -> i32 {
    return a + b;
}
