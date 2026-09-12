struct File { fd: i32 }

trait Drop {
    fn drop(self: &rw Self);
}

impl Drop for File {
    fn drop(self: &rw Self) {}
}

fn consume(f: File) {}

fn main() {
    dec f = File { fd: 1 }; //~ ERROR Cannot drop conditionally moved value
    dec cond = true;
    
    if cond {
        consume(f); // f moved here
    } else {
        // f not moved here
    }
    // Is f moved here?
}
