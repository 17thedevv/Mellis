struct File {
    fd: i32,
}

trait Drop {
    fn drop(self: &rw Self);
}

impl Drop for File {
    fn drop(self: &rw Self) {}
}

fn main() {
    dec f = File { fd: 1 };
    dec x = f;
    dec y = f; //~ ERROR Use of moved value
}
