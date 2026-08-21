// Test advanced patterns: fn type params + @<T> generic intrinsics

intrinsic fn unreachable() -> void;

export enum Option<T> {
    Some(T),
    None,
}

// Test: fn type parameter in generic method + higher-kinded result
impl<T> Option<T> {
    fn map<U>(self, f: fn (T) -> U) -> Option<U> {
        match self {
            Option::Some(v) -> Option<U>::Some(f(v)),
            Option::None -> Option<U>::None
        }
    }

    fn unwrap_or(self, def: T) -> T {
        match self {
            Option::Some(v) -> v,
            Option::None -> def
        }
    }
}

fn add_one(x: int_32) -> int_32 {
    return x + 1;
}

fn main() -> int_32 {
    dec some_41: Option<int_32> = Option::Some(41);
    dec mapped: Option<int_32> = some_41.map@<int_32>(add_one);
    dec result: int_32 = mapped.unwrap_or(0);
    if result != 42 { return 1; }

    dec none_val: Option<int_32> = Option::None;
    dec fallback: int_32 = none_val.unwrap_or(99);
    if fallback != 99 { return 2; }

    return 0;
}
