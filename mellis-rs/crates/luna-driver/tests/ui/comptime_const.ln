// Comptime const declaration and usage test

const BUFFER_CAP: i32 = 100 + 28; //~ ERROR cannot evaluate constant in comptime
const IS_ACTIVE: bool = true; //~ ERROR cannot evaluate constant in comptime

fn main() -> i32 {
    dec size = BUFFER_CAP;
    if IS_ACTIVE {
        size + 1
    } else {
        0
    }
}
