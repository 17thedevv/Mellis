// Test: using must target namespace, not leaf symbol
// Expected: error — Vec is a struct, not a module

module std {
    export struct Vec {
        len: i32,
    }
}

using std::Vec as V;

fn main() -> i32 {
    return 0;
}
