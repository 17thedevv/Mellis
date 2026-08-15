struct MyInt {}

enum Option<T> {
    Some(T),
    None,
}

fn test_nested(opt: Option<Option<MyInt>>) -> MyInt {
    match opt {
        Option::Some(Option::Some(v)) -> v,
        Option::Some(Option::None) -> 0,
        Option::None -> -1
    }
}

fn main() -> MyInt {
    dec a = Option::Some(Option::Some(42));
    dec b = Option::Some(Option::None);
    dec c = Option::None;
    
    return test_nested(a) + test_nested(b) + test_nested(c);
}
