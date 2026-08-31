// Test: using nested leaf symbol must be rejected
// Expected: error — Vec is a struct, not a module

module std {
    export module collections {
        export struct Vec {
            len: i32,
        }
    }
}

using std::collections::Vec as v;

fn main() -> i32 {
    return 0;
}
