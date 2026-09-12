extern fn printf(format: str, ...) -> int_32;

mod mylib;
use mylib::my_add;

fn main() -> int_32 {
    dec result = my_add(10, 20);
    printf("Result is %d\n", result);
    return result;
}
