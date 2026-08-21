// ==========================================
// test_comptime.ms
// Tests for compile-time evaluation features
// ==========================================

// A comptime function that performs math
comptime fn factorial(n: i32) -> i32 {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

comptime fn add_five(x: i32) -> i32 {
    return x + 5;
}

fn main() {
    // 1. Basic comptime block
    comptime {
        dec x = 10;
        dec y = 20;
        dec z = x + y;
    }

    // 2. Call comptime function in comptime block
    comptime {
        dec result = add_five(10);
    }

    // 3. Comptime function recursion
    comptime {
        dec fact5 = factorial(5); // should be 120
    }
}
