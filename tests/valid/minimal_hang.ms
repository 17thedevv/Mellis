struct Vec<T> { ptr: *rw T }
fn vec_new<T>() -> Vec<T> { Vec { ptr: 0 as *rw T } }
fn main() { dec v = vec_new@<int_32>(); }
