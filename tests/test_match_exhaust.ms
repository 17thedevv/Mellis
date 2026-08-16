enum Option {
    Some(i32),
    None,
}

fn test_option(opt: Option) -> i32 {
    match opt {
        Option::Some(val) -> val,
        Option::None -> 0,
    }
}

enum Result {
    Ok(Option),
    Err(i32),
}

fn test_nested(res: Result) -> i32 {
    match res {
        Result::Ok(Option::Some(v)) -> v,
        Result::Ok(Option::None) -> 0,
        Result::Err(e) -> e,
    }
}

fn test_missing(res: Result) -> i32 {
    match res {
        Result::Ok(Option::Some(v)) -> v,
        Result::Err(e) -> e,
    }
}

fn main() {
    test_option(Option::Some(42));
}
