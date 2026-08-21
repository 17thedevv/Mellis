// test_labeled_break.ms
// Test nested labeled break and continue

fn main() -> i32 {
    dec sum = 0;
    dec i = 0;
    
    'outer: while (i < 5) {
        dec j = 0;
        'inner: while (j < 5) {
            if (i == 2) {
                if (j == 2) {
                    break 'outer;
                }
            }
            if (j == 3) {
                i = i + 1;
                continue 'outer;
            }
            sum = sum + 1;
            j = j + 1;
        }
        i = i + 1;
    }
    
    // i=0: j=0..2 (3) => j=3 continue outer. sum = 3
    // i=1: j=0..2 (3) => j=3 continue outer. sum = 6
    // i=2: j=0..1 (2) => j=2 break outer. sum = 8
    
    if (sum != 8) {
        return 1;
    }
    
    return 0;
}
