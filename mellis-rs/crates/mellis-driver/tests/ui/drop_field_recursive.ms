struct File {
    fd: i32
}

trait Drop { fn drop(self: &rw Self); }

impl Drop for File {
    fn drop(self: &rw Self) {}
}

struct Wrapper {
    a: File,
    b: File
}

fn main() {
    dec w = Wrapper {
        a: File { fd: 1 },
        b: File { fd: 2 }
    };
    // The compiler should recursively emit Drop for w.b then w.a
}
