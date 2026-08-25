struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn get_file() -> File {
    dec x = File { fd: 1 };
    dec y = x; // x is moved
    return y;  // y is returned, not dropped
}

fn main() {
    dec f = get_file();
}
