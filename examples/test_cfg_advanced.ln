extern fn printf(format: str, ...) -> int_32;

fn test_cstyle_for() -> int_32 {
    dec sum = 0;
    for (dec i = 0; i < 10; i = i + 1) {
        if (i == 5) {
            continue;
        }
        if (i == 8) {
            break;
        }
        sum = sum + i;
    }
    return sum; // 0 + 1 + 2 + 3 + 4 + 6 + 7 = 23
}

fn test_while() -> int_32 {
    dec sum = 0;
    dec i = 0;
    while (i < 10) {
        if (i == 5) {
            i = i + 1;
            continue;
        }
        if (i == 8) {
            break;
        }
        sum = sum + i;
        i = i + 1;
    }
    return sum; // 0 + 1 + 2 + 3 + 4 + 6 + 7 = 23
}

fn main() -> int_32 {
    dec sum1 = test_cstyle_for();
    printf("C-style for sum: %d\n", sum1);
    
    dec sum2 = test_while();
    printf("While sum: %d\n", sum2);

    return 0;
}
