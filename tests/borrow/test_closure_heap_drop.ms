extern fn printf(fmt: str, ...) -> int_32;
extern fn fflush(stream: *int_32) -> int_32;

struct Resource {
    val: int_32;
}

trait Drop {
    fn drop(self: &rw Resource);
}

impl Drop for Resource {
    fn drop(self: &rw Resource) {
        printf("Resource dropped\n");
    }
}

fn main() -> int_32 {
    printf("Creating closure...\n");
    dec c;
    {
        dec r = Resource { val: 42 };
        c = move | | -> void {
            printf("Inside closure: %d\n", r.val);
        };
    }
    
    printf("Calling closure...\n");
    c();
    
    printf("Dropping closure...\n");
    return 0;
}
