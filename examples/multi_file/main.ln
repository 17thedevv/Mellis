// main.fl
extern fn printf(format: *int_8, ...) -> int_32;

mod math;
use math::add;

export fn main() -> int_32 {
    dec result = add(10, 20);
    printf("10 + 20 = %d\n", result);
    return 0;
}
