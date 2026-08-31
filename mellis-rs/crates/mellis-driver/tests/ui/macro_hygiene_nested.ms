macro inner {
    () => {
        dec tmp = 1;
    }
}

macro outer {
    () => {
        dec outer_var = 10;
        inner!();
    }
}

fn main() -> i32 {
    dec tmp = 42;
    outer!();
    return tmp;
}
