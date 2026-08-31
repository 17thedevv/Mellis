#[derive(Clone, PartialEq, Default)]
enum OptionI32 {
    Some(i32),
    None,
}

fn main() -> i32 {
    dec x = OptionI32::Some(42);
    dec y = x.clone();
    
    if x.eq(&y) {
        return 1;
    }
    return 0;
}
