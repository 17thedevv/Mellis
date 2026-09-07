export module alloc {
    #[link(name = "malloc")]
    export intrinsic fn heap_alloc(size: i32) -> *i32;
}
