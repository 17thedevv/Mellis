struct File { fd: i32 }
trait Drop { fn drop(self: &rw Self); }
impl Drop for File { fn drop(self: &rw Self) {} }
fn consume(f: File) {}

fn main() {
    dec f = File { fd: 1 }; //~ ERROR Cannot drop conditionally moved value
    
    while true {
        consume(f); // Move in loop
        break;      // Unconditional break
    }
}
