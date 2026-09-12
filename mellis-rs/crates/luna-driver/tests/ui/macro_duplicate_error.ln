macro test_dup {
    (@x: expr) => { @x }
}

macro test_dup { //~ ERROR Duplicate definition of symbol `test_dup` in the same scope
    (@x: expr) => { @x * 2 }
}

fn main() -> i32 {
    return 0;
}
