intrinsic fn str_len(s: str) -> uint_64;
intrinsic fn str_ptr(s: str) -> *i8;

mod core {
    export mod strmod {
        export fn len(s: str) -> uint_64 {
            return str_len(s);
        }
    }
}

fn main() -> int_32 {
    dec s: str = "Hello World!";
    dec length: uint_64 = core::strmod::len(s);
    return length as int_32;
}
