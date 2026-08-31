// Negative test: comptime division by zero

const BAD: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    dec x = 10;
    x / 0
};

fn main() -> i32 {
    BAD
}
