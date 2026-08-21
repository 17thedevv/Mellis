// Consumer test for core StdLib.
// Imports from core.mlib (built via: mellis core.ms -lib)
//
// Compile command:
//   mellis consumer_core.ms -L lib/core --emit-exe -o build/consumer_core.exe

use core::{Option, Result, Clone, Drop, Eq, Ord, Ordering};

fn main() -> int_32 {
    // ---- Option<int_32> monomorphization tests ----
    dec some_v: Option<int_32> = Option::Some(42);
    if some_v.is_none() { return 1; }
    dec a: int_32 = some_v.unwrap();
    if a != 42 { return 2; }

    dec none_v: Option<int_32> = Option::None;
    if none_v.is_some() { return 3; }
    dec b: int_32 = none_v.unwrap_or(99);
    if b != 99 { return 4; }

    // ---- Result<int_32, int_32> monomorphization tests ----
    dec ok_v: Result<int_32, int_32> = Result::Ok(7);
    if ok_v.is_err() { return 5; }
    dec c: int_32 = ok_v.unwrap();
    if c != 7 { return 6; }

    dec err_v: Result<int_32, int_32> = Result::Err(-10);
    if err_v.is_ok() { return 7; }
    dec d: int_32 = err_v.unwrap_or(0);
    if d != 0 { return 8; }

    // Note: Additional methods (ok, err, unwrap_err) known to require
    // TypeChecker cross-enum unification improvements. They are part of
    // the core.mlib interface and will be testable once the TypeChecker
    // handles impls that return other generic enum types.

    return 0;
}
