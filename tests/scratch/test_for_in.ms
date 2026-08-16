// test_for_in.ms — For-In loop semantic tests

// ── Test 1: Array intrinsic path ─────────────────────────────────────────────
// for (x in arr) where arr is [T; N] → index iteration

fn sum_array() -> int_32 {
    dec arr = [1, 2, 3, 4, 5];
    dec sum: int_32 = 0;
    for (x in arr) {
        sum = sum + x;
    }
    return sum;  // expect 15
}

fn main() -> int_32 {
    return sum_array();
}

// ── Test 2: Nested for-in ────────────────────────────────────────────────────
// (Compile-only; should produce correct loop CFG with nested iterators)
// fn nested() -> int_32 {
//     dec outer = [1, 2];
//     dec result: int_32 = 0;
//     for (x in outer) {
//         dec inner = [10, 20];
//         for (y in inner) {
//             result = result + y;
//         }
//     }
//     return result;  // expect 60
// }

// ── Semantic edge cases (compile-error tests) ─────────────────────────────────
// These should each produce a diagnostic error:

// for (x in 5) { }          // Error: i32 is not iterable
// for (x in true) { }       // Error: bool is not iterable
