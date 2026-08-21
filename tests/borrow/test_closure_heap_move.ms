extern fn printf(fmt: str, ...) -> int_32;

struct Resource {
    val: int_32;
}

trait Drop {
    fn drop(self: &rw Resource);
}

impl Drop for Resource {
    fn drop(self: &rw Resource) {
        printf("Resource dropped: %d\n", self.val);
    }
}

fn main() -> int_32 {
    dec f;
    {
        dec r = Resource { val: 42 };
        f = move | | -> void {
            printf("Inside closure: %d\n", r.val);
        };
    }
    printf("Closure created.\n");
    
    dec g = f; // Move f into g
    printf("Closure moved to g.\n");
    
    g(); // Call g. g should drop the resource.
    
    printf("End of main.\n");
    return 0;
}
