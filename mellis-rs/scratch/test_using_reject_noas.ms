// Test: using without 'as' must be rejected
// Expected: parse error

module std {
    export module collections {}
}

using std::collections;

fn main() -> i32 {
    return 0;
}
