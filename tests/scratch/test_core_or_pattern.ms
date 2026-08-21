// Test: Option::None with explicit @<T> generic arg on enum variant value path
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
    // or: return self if Some, else optb
    fn or(self, optb: Option<T>) -> Option<T> {
        match self {
            Option::Some(v) -> Option::Some(v),
            Option::None -> optb
        }
    }
}

fn main() -> int_32 {
    // Test 1: Some().or(None) → Some
    dec a: Option<int_32> = Option::Some(5);
    dec b: Option<int_32> = Option::None;
    dec c: Option<int_32> = a.or(b);
    if !c.is_some() { return 1; }
    dec cv: int_32 = c.unwrap();
    if cv != 5 { return 2; }

    // Test 2: None.or(Some(10)) → Some(10)
    dec d: Option<int_32> = Option::None;
    dec e: Option<int_32> = Option::Some(10);
    dec f: Option<int_32> = d.or(e);
    dec fv: int_32 = f.unwrap_or(-1);
    if fv != 10 { return 3; }

    return 0;
}
