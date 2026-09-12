struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn get_file() -> File {
    dec f = File { fd: 1 };
    return f;
}

fn main() {
    dec f = get_file();
}
