struct Vec {
    data: &i32,
}

struct VecIter {
    ptr: &i32,
}

impl Vec {
    fn iter(self: &Vec) -> VecIter {
        return VecIter { ptr: self.data };
    }
}

impl VecIter {
    fn next(self: &rw VecIter) -> &i32 {
        return self.ptr;
    }
}

fn main() {
    dec rw val = 10;
    dec rw vec = Vec { data: &val };
    
    dec rw iterator = vec.iter();
    dec item = iterator.next();
    
    // Should fail because `val` is borrowed via `item` (derived from `iterator`, derived from `vec`, derived from `val`)
    val = 20;
    
    dec w = item;
}
