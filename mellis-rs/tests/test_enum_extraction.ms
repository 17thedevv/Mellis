enum MyOption {
    None,
    Some(i32),
}

enum Wrapper {
    Empty,
    Value(MyOption),
}

fn unwrap_nested(w: Wrapper) -> i32 {
    match w {
        Wrapper::Value(MyOption::Some(x)) -> x,
        Wrapper::Value(MyOption::None) -> 0,
        Wrapper::Empty -> -1,
    }
}

fn main() -> i32 {
    dec a = Wrapper::Value(MyOption::Some(42));
    dec b = Wrapper::Value(MyOption::None);
    dec c = Wrapper::Empty;
    
    dec x = unwrap_nested(a);
    dec y = unwrap_nested(b);
    dec z = unwrap_nested(c);
    
    // Should be 42 + 0 + (-1) = 41
    x + y + z
}
