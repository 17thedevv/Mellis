#[repr(C)]
struct FfiSafe {
    x: i32,
}

struct NotFfiSafe {
    x: i32,
}

// Function that takes an FFI safe struct is fine if not exported as C ABI, but 
// let's just make sure #[repr(C)] compiles without errors.

fn main() -> i32 {
    dec a = FfiSafe { x: 10 };
    dec b = NotFfiSafe { x: 20 };
    return a.x + b.x;
}
