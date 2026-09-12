// Negative test: calling extern/impure function in comptime is forbidden

extern fn puts(s: *u8) -> i32;

const BAD: i32 = comptime { //~ ERROR cannot evaluate constant in comptime
    puts(cast(*u8) 0)
};

fn main() -> i32 {
    BAD
}
