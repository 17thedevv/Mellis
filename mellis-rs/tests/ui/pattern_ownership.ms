// native_pattern.ms

export struct Point {
    x: int,
    y: int,
}

export enum Option {
    Some(Point),
    None,
}

@[no_mangle]
export fn main() -> int {
    // Case 1: Partial move and use of remaining field
    dec p = Point { x: 10, y: 20 };
    dec Point { x } = p; 
    
    // Use the remaining field after a partial move of `x`.
    // `x` is 10, `p.y` is 20. Total should be 30.
    dec sum1 = x + p.y;
    
    // Case 2: Nested pattern matching (Enum -> Struct)
    dec nested = Option::Some(Point { x: 1, y: 5 });
    dec rw sum2 = 0;
    
    match nested {
        Option::Some(Point { x: 1, y }) -> {
            sum2 = y; // y should be 5
        }
        Option::Some(_) -> {
            sum2 = 100;
        }
        Option::None -> {
            sum2 = 200;
        }
    };

    // Verification output
    // We will return sum1 + sum2, which should be 30 + 5 = 35.
    dec total = sum1 + sum2;
    return total;
}
