extern fn printf(fmt: str, ...) -> int_32;
extern fn malloc(size: int_64) -> *int_32;
extern fn free(p: *int_32) -> void;

fn main() -> int_32 {
    printf("Allocating\n");
    dec p = malloc(16 as int_64);
    printf("Freeing\n");
    free(p);
    printf("Done\n");
    return 0;
}
