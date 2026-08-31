// Negative test: reading uninitialized variable in comptime is caught

const BAD: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    dec x: i32;
    x
};

fn main() -> i32 {
    0
}
