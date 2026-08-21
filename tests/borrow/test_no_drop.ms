extern fn printf(fmt: str, ...) -> int_32;
fn main() -> int_32 {
    printf("Before closure call\n");
    dec c;
    {
        dec x = 42;
        c = move | | -> void {
            printf("Inside closure: %d\n", x);
        };
    }
    printf("Before c()\n");
    c();
    printf("After c()\n");
    return 0;
}
