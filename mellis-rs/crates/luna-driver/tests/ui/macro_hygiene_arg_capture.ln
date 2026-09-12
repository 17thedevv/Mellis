macro add_with_internal {
    (@input: expr, @target: ident) => {
        dec x: i32 = 100;
        @target = @input + x;
    }
}

fn main() -> i32 {
    dec x: i32 = 5;
    dec rw res: i32 = 0;
    add_with_internal!(x * 2, res);
    return res;
}
