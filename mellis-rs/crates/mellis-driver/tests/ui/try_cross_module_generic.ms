import <core>;

fn generic_try<T, E>(x: core::Result<T, E>) -> core::Result<T, E> {
    dec val = x?;
    return core::Result::Ok(val);
}

fn main() -> i32 {
    dec res: core::Result<i32, i32> = core::Result::Ok(10);
    dec final_res = generic_try(res);
    return 0;
}
