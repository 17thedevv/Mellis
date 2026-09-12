enum MyOption {
    Some(int_32),
    None
}

export fn main() -> int_32 {
    dec opt = MyOption::Some(42);
    dec result = match opt {
        MyOption::Some(val) -> val,
        MyOption::None -> 0
    };
    return result;
}
