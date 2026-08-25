struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn main() {
    dec f = File { fd: 1 };
    dec f2 = File { fd: 2 };
    // f2 and f should drop implicitly here
}
