use core::{Option, Result, Clone, Drop, Eq};
intrinsic fn ptr_write<T>(p: *rw T, val: T);
intrinsic fn ptr_read<T>(p: *const T) -> T;

struct Tracker {
    value: int_32;
    dropped: *rw bool;
}

impl Drop for Tracker {
    fn drop(self: *rw Self) {
        unsafe {
            ptr_write(self.dropped, true);
        }
    }
}

fn test_option() -> int_32 {
    dec some_v: Option<int_32> = Option::Some(42);
    if some_v.is_none() { return 1; }
    dec a: int_32 = some_v.unwrap();
    if a != 42 { return 2; }

    dec none_v: Option<int_32> = Option::None;
    if none_v.is_some() { return 3; }
    dec b: int_32 = none_v.unwrap_or(99);
    if b != 99 { return 4; }

    return 0;
}

fn test_result() -> int_32 {
    dec ok_v: Result<int_32, int_32> = Result::Ok(7);
    if ok_v.is_err() { return 1; }
    dec c: int_32 = ok_v.unwrap();
    if c != 7 { return 2; }

    dec err_v: Result<int_32, int_32> = Result::Err(-10);
    if err_v.is_ok() { return 3; }
    dec d: int_32 = err_v.unwrap_or(0);
    if d != 0 { return 4; }

    return 0;
}

fn test_drop() -> int_32 {
    dec is_dropped = false;
    {
        dec opt: Option<Tracker> = Option::Some(Tracker { value: 100, dropped: &rw is_dropped as *rw bool });
    }
    // Tracker inside Option should have been dropped.
    if !is_dropped { return 1; }

    dec is_dropped_err = false;
    {
        dec res: Result<int_32, Tracker> = Result::Err(Tracker { value: 200, dropped: &rw is_dropped_err as *rw bool });
    }
    // Tracker inside Result should have been dropped.
    if !is_dropped_err { return 2; }

    return 0;
}

fn main() -> int_32 {
    dec err1 = test_option();
    if err1 != 0 { return 10 + err1; }

    dec err2 = test_result();
    if err2 != 0 { return 20 + err2; }

    dec err3 = test_drop();
    if err3 != 0 { return 30 + err3; }

    return 0;
}
