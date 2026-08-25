struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn consume_file(f: File) {
    // f is dropped here (first and only time)
}

fn main() {
    dec f = File { fd: 1 };
    consume_file(f); // f is moved
    // f should NOT be dropped here again
}
