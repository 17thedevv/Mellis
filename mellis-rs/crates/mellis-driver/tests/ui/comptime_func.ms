// Comptime function call test

fn factorial(n: i32) -> i32 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

const FACT_6: i32 = comptime {
    factorial(6)
};

fn main() -> i32 {
    FACT_6
}
