// Test: use the auto-visible Option contract and Option::is_some method

fn main() -> i32 {
    dec x: Option<i32> = Option::Some(42);
    if x.is_some() {
        return 1;
    }
    return 0;
}
