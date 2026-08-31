// Comptime const declaration and usage test

const BUFFER_CAP: i32 = 100 + 28;
const IS_ACTIVE: bool = true;

fn main() -> i32 {
    dec size = BUFFER_CAP;
    if IS_ACTIVE {
        size + 1
    } else {
        0
    }
}
