// lib/alloc/allocator.ms
// Mellis Allocator trait — interface between alloc and runtime memory ABI.
// The concrete allocator calls __mellis_alloc / __mellis_dealloc via FFI.

extern fn __mellis_alloc(size: uint_64, align: uint_64) -> *rw uint_8;
extern fn __mellis_dealloc(ptr: *rw uint_8, size: uint_64, align: uint_64);
extern fn __mellis_realloc(ptr: *rw uint_8, old_size: uint_64, new_size: uint_64, align: uint_64) -> *rw uint_8;

export trait Allocator {
    fn alloc(self, size: uint_64, align: uint_64) -> *rw uint_8;
    fn dealloc(self, ptr: *rw uint_8, size: uint_64, align: uint_64);
}

export struct GlobalAlloc {}

impl Allocator for GlobalAlloc {
    fn alloc(self, size: uint_64, align: uint_64) -> *rw uint_8 {
        return __mellis_alloc(size, align);
    }
    fn dealloc(self, ptr: *rw uint_8, size: uint_64, align: uint_64) {
        __mellis_dealloc(ptr, size, align);
    }
}
