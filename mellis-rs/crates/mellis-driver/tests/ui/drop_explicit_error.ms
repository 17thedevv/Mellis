struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }

fn main() {
    dec rw f = File { fd: 1 };
    drop(&rw f); //~ ERROR Explicit calls to drop() are forbidden. Values are dropped automatically at end of scope. //~ ERROR Symbol 'drop' not found
}
