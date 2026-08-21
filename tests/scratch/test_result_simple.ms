// Even smaller subset - avoid unwrap_err for now
intrinsic fn unreachable() -> void;

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
    fn unwrap_or(self, def: T) -> T {
        match self {
            Result::Ok(v) -> v,
            Result::Err(e) -> def
        }
    }
}

fn main() -> int_32 {
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
