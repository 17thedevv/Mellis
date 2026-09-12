fn calc_match(x: i32) -> i32 {
    dec res: i32 = 0;
    match x {
        1 -> { res = 10; }
        2 -> { res = 20; }
        _ -> { res = 100; }
    };
    return res;
}

export fn main() -> i32 {
    dec sum: i32 = 0;
    dec i: i32 = 1;

    // While loop: sum from 1 to 10
    while i <= 10 {
        if i == 5 {
            sum = sum + 50; // extra bonus
        } else {
            sum = sum + i;
        }
        i = i + 1;
    }

    // sum should be: (1+2+3+4+6+7+8+9+10) + 50 = 50 + 50 = 100

    dec m1: i32 = calc_match(1); // 10
    dec m2: i32 = calc_match(2); // 20
    dec m3: i32 = calc_match(99); // 100

    return sum + m1 + m2 + m3; // 100 + 10 + 20 + 100 = 230
}
