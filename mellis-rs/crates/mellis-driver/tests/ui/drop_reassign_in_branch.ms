struct File { fd: i32 }


impl Drop for File {
    fn drop(self: &rw Self) {}
}

fn consume(f: File) {}
fn use_file(f: &rw File) {}

fn main() {
    dec rw f = File { fd: 1 };
    dec cond = true;
    
    if cond {
        consume(f); // f is moved
        f = File { fd: 2 }; // f is reassigned (Live again)
    } else {
        // f is not moved, remains Live
    }
    
    // At this point, f should be safely Live!
    use_file(&rw f);
}
