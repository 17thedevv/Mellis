// EXACT reproduction of the working test_core_full_simple.ms main() order
intrinsic fn unreachable() -> void;

enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    fn is_some(self: &Self) -> bool {
        match self {
            Option::Some(v) -> true,
            Option::None -> false
        }
    }
    fn is_none(self: &Self) -> bool {
        return !self.is_some();
    }
    fn unwrap(self) -> T {
        match self {
            Option::Some(v) -> v,
            Option::None -> { unsafe { unreachable(); } }
        }
    }
    fn unwrap_or(self, def: T) -> T {
        match self {
            Option::Some(v) -> v,
            Option::None -> def
        }
    }
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    fn is_ok(self: &Self) -> bool {
        match self {
            Result::Ok(v) -> true,
            Result::Err(e) -> false
        }
    }
    fn is_err(self: &Self) -> bool {
        return !self.is_ok();
    }
    fn unwrap(self) -> T {
        match self {
            Result::Ok(v) -> v,
            Result::Err(e) -> { unsafe { unreachable(); } }
        }
    }
    fn unwrap_err(self) -> E {
        match self {
            Result::Ok(v) -> { unsafe { unreachable(); } },
            Result::Err(e) -> e
        }
    }
    fn unwrap_or(self, def: T) -> T {
        match self {
            Result::Ok(v) -> v,
            Result::Err(e) -> def
        }
    }
    fn ok(self) -> Option<T> {
        match self {
            Result::Ok(v) -> Option::Some(v),
            Result::Err(e) -> Option::None
        }
    }
    fn err(self) -> Option<E> {
        match self {
            Result::Ok(v) -> Option::None,
            Result::Err(e) -> Option::Some(e)
        }
    }
}

fn main() -> int_32 {
    // ---- Option tests ----
    dec some_v: Option<int_32> = Option::Some(42);
    if some_v.is_none() { return 1; }
    dec a: int_32 = some_v.unwrap();
    if a != 42 { return 2; }

    dec none_v: Option<int_32> = Option::None;
    if none_v.is_some() { return 3; }
    dec b: int_32 = none_v.unwrap_or(99);
    if b != 99 { return 4; }

    // ---- Result tests ----
    dec ok_v: Result<int_32, int_32> = Result::Ok(7);
    if ok_v.is_err() { return 5; }
    dec c: int_32 = ok_v.unwrap();
    if c != 7 { return 6; }

    dec err_v: Result<int_32, int_32> = Result::Err(-10);
    if err_v.is_ok() { return 7; }
    dec d: int_32 = err_v.unwrap_or(0);
    if d != 0 { return 8; }
    dec err_code: int_32 = err_v.unwrap_err();
    if err_code != -10 { return 9; }

    // Result -> Option
    dec opt_from_ok: Option<int_32> = ok_v.ok();
    if opt_from_ok.is_none() { return 10; }
    dec e: int_32 = opt_from_ok.unwrap();
    if e != 7 { return 11; }

    dec err_from_err: Option<int_32> = err_v.err();
    if err_from_err.is_none() { return 12; }
    dec f: int_32 = err_from_err.unwrap();
    if f != -10 { return 13; }

    // ok() on Err gives None
    dec opt_from_err: Option<int_32> = err_v.ok();
    if !opt_from_err.is_none() { return 14; }

    // err() on Ok gives None
    dec none_err: Option<int_32> = ok_v.err();
    if !none_err.is_none() { return 15; }

    return 0;
}
