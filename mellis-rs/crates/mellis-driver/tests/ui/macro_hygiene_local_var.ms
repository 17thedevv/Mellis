macro set_internal {
    (@val: expr) => {
        dec tmp: i32 = @val;
    }
}

fn main() -> i32 {
    dec tmp: i32 = 10;
    set_internal!(20);
    return tmp;
}
