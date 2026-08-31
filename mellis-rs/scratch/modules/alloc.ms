
module std {
    export struct Vec<T> {
        ptr: *rw T,
        len: i32,
        cap: i32,
    }
}
