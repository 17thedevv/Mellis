extern fn record_drop_called(fd: i32);

struct File {
    fd: i32
}

trait Drop { fn drop(self: &rw Self); }

impl Drop for File {
    fn drop(self: &rw Self) {
        record_drop_called(self.fd);
    }
}

struct Mixed {
    a: File,
    b: i32
}

fn main() {
    dec m = Mixed {
        a: File { fd: 999 },
        b: 42
    };
    // Expected:
    // Mvir should show `record_drop_called(999)` being reached through `File::drop`.
}
