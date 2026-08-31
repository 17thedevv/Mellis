struct Vec {
    data: &rw i32,
}
struct VecIterMut {
    ptr: &rw i32,
}
impl Vec {
    fn iter_mut(self: &rw Vec) -> VecIterMut {
        return VecIterMut { ptr: self.data };
    }
}
impl VecIterMut {
    fn next(self: &rw VecIterMut) -> &rw i32 {
        return self.ptr;
    }
}
fn main() {
    dec rw val = 10;
    dec rw vec = Vec { data: &rw val };
    
    dec rw it = vec.iter_mut();
    
    dec a = it.next();
    dec b = it.next();
    
    dec v1 = a;
    dec v2 = b;
}
