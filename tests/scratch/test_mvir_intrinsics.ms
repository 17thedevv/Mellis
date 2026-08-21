intrinsic fn unreachable() -> void;
intrinsic fn ptr_read(p: *const int_32) -> int_32;
intrinsic fn ptr_write(p: *rw int_32, val: int_32) -> void;
intrinsic fn mem_copy(src: *const int_32, dest: *rw int_32, count: int_64) -> void;

fn main() {
    dec x: int_32 = 10;
    unsafe {
        dec p = &rw x as *rw int_32;
        ptr_write(p, 20);
        
        dec y = ptr_read(p as *const int_32);
        
        dec z: int_32 = 0;
        mem_copy(p as *const int_32, &rw z as *rw int_32, 1 as int_64);
        
        if y != 20 {
            unreachable();
        }
        if z != 20 {
            unreachable();
        }
    }
}
