struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn main() {
    dec rw i = 0;
    while i < 2 {
        dec f = File { fd: 1 };
        i = i + 1;
        continue;
    }
}
