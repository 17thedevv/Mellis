struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn main() {
    dec x = 1;
    if x == 1 {
        dec f = File { fd: 1 };
        // f is dropped here
    }
}
