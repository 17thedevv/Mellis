// test_try.ms
// Tests for the ? (try) operator on Option
// Uses ONLY existing Mellis syntax confirmed from lib/core and e2e tests.

extern fn __mellis_panic();

enum Option<T> {
    None,
    Some(T),
}

impl<T> Option<T> {
    fn is_error(self: Self) -> bool {
        match self {
            Option::None -> true,
            Option::Some(_) -> false,
        }
    }

    fn extract_error(self: Self) -> () {
        ()
    }

    fn unwrap(self: Self) -> T {
        match self {
            Option::Some(val) -> val,
            Option::None -> {
                __mellis_panic()
            }
        }
    }

    fn from_error(err: ()) -> Option<T> {
        Option::None
    }
}

fn test_try(val: Option<int_32>) -> Option<int_32> {
    dec x = val?;
    return Option::Some(x + 10);
}

fn main() -> int_32 {
    dec a = Option::Some(5);
    dec b = test_try(a);
    return 0;
}
