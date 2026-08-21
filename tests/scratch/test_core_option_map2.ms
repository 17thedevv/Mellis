// Test Option map/and_then/unwrap_or with @<T> explicit generic syntax

intrinsic fn unreachable() -> void;

export enum Option<T> {
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
            Option::None -> {
                unsafe { unreachable(); }
            }
        }
    }

    fn unwrap_or(self, def: T) -> T {
        match self {
            Option::Some(v) -> v,
            Option::None -> def
        }
    }

    fn map<U>(self, f: fn (T) -> U) -> Option<U> {
        match self {
            Option::Some(v) -> Option<U>::Some(f(v)),
            Option::None -> Option<U>::None
        }
    }
}

fn add_one(x: int_32) -> int_32 {
    return x + 1;
}

fn main() -> int_32 {
    // 1. Some + unwrap
    dec some_41: Option<int_32> = Option::Some(41);
    if !some_41.is_some() { return 1; }
    dec v1: int_32 = some_41.unwrap();
    if v1 != 41 { return 2; }

    // 2. None + unwrap_or fallback
    dec none_val: Option<int_32> = Option::None;
    if !none_val.is_none() { return 3; }
    dec fallback: int_32 = none_val.unwrap_or(99);
    if fallback != 99 { return 4; }

    // 3. map on Some — use @<int_32> explicit generic arg for map's U
    //    explicit @<U> arg goes after method name, then fn arg
    dec some_7: Option<int_32> = Option::Some(7);
    dec mapped: Option<int_32> = some_7.map@<int_32>(add_one);
    dec v2: int_32 = mapped.unwrap_or(0);
    if v2 != 8 { return 5; }

    // 4. map on None — stays None
    dec none_2: Option<int_32> = Option::None;
    dec mapped_none: Option<int_32> = none_2.map@<int_32>(add_one);
    if !mapped_none.is_none() { return 6; }

    return 0;
}
