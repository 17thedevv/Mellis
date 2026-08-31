// Test: export using must be rejected
// Expected: parse error

module std {
    export module collections {}
}

export using std::collections as col;

fn main() -> i32 {
    return 0;
}
