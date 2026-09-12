fn main() -> int_32 {
    dec x = 10;
    dec outer = | | -> void {
        dec inner = | | -> void {
            x = x + 1;
        };
        inner();
    };
    outer();
    return x;
}
