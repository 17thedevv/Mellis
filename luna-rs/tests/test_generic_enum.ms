fn main() -> i32 {
    dec a: Option<i32> = Option::Some(42);
    
    match a {
        Option::Some(x) -> x,
        Option::None -> 0,
    }
}
