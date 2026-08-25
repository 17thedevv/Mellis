struct Node {
    value: i32,
    next: *rw Node,
}

trait Drop { fn drop(self: &rw Self); }
impl Drop for Node { fn drop(self: &rw Self) {} }

fn test_drop(n: Node) {
    // n will be dropped at the end of this function
    // and needs_drop(Node) will be called, testing cycle safety
}

fn main() {}
