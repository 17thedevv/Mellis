enum MyOption {
    None,
    Some(i32),
}

fn unwrap(opt: MyOption) -> i32 {
    match opt {
        MyOption::Some(a, b) -> a, // Error: wrong arity
        MyOption::None -> 0,
    }
}

fn main() -> i32 {
    0
}
