type Ptr<T> = *rw T;

macro ptr {
    (@t: ty) => { Ptr<@t> }
}

fn foo(x: ptr!(i32)) -> ptr!(i32) {
    return x;
}

fn main() -> i32 {
    dec rw val: i32 = 42;
    dec p: ptr!(i32) = &rw val;
    dec out = foo(p);
    return 0;
}
