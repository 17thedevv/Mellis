struct File {
    fd: i32
}

trait Drop { fn drop(self: &rw Self); }

impl Drop for File {
    fn drop(self: &rw Self) {}
}

struct Mixed {
    a: File,
    b: i32
}

fn main() {
    dec m = Mixed {
        a: File { fd: 1 },
        b: 42
    };
    // The compiler should only emit extract + drop for m.a, and do nothing for m.b
}
