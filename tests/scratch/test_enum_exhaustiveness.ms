enum Option {
    Some(int_32),
    None
}

fn main() {
    dec opt = Option::Some(42);
    
    // ERROR: non-exhaustive match (missing None)
    dec val = match opt {
        Option::Some(x) -> x
    };
}
