// Test: using namespace alias — valid case
// Expected: no errors

module std {
    export module collections {
        export struct Vec {
            len: i32,
        }
    }
}

using std::collections as col;

fn main() -> i32 {
    dec v: col::Vec;
    return 0;
}
