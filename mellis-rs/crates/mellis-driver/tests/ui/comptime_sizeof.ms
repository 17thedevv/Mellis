// Comptime sizeof and alignof test

fn main() -> i32 {
    dec s_i32 = comptime { sizeof(i32) };
    dec s_bool = comptime { sizeof(bool) };
    dec s_f64 = comptime { sizeof(f64) };
    dec a_f64 = comptime { alignof(f64) };

    s_i32 + s_bool + s_f64 + a_f64
}
