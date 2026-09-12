// Phase 9.4.1: Overlapping &rw borrows must be rejected.
//
// dec rw it = vec.iter_mut();
// dec a = it.next();
// dec b = it.next();  // ERROR: a is still alive
// use(a);             // <-- proves a is alive when b borrows
// use(b);
//
// This is the critical uniqueness test.

struct Vec<T> {
    data: *rw T,
    len: i32,
    cap: i32,
}


struct VecIter<T> {
    ptr: *rw T,
}

enum Option<T> {
    None,
    Some(T),
}

impl<T> Vec<T> {
    fn iter_mut(self: &rw Vec<T>) -> VecIter<T> {
        dec rw iter: VecIter<T>;
        iter.ptr = self.data;
        return iter;
    }
}

impl<T> VecIter<T> {
    fn next(self: &rw VecIter<T>) -> Option<&rw T> {
        return Option::Some(self.ptr);
    }
}

fn use_val<T>(val: &rw T) {}

fn get_vec() -> Vec<i32> {
    dec rw dummy = 0;
    return Vec<i32> {
        data: &rw dummy,
        len: 0,
        cap: 0,
    };
}

fn main() {
    dec rw vec: Vec<i32> = get_vec();
    dec rw iter = vec.iter_mut();

    dec a_opt = iter.next();
    dec b_opt = iter.next();

    // Both alive simultaneously - proves the conflict
    match a_opt {
        Option::Some(a) -> use_val<i32>(a),
        Option::None -> {},
    };

    match b_opt {
        Option::Some(b) -> use_val<i32>(b),
        Option::None -> {},
    };
}
