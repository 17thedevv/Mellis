macro assign_val {
    (@x: ident = @y: expr) => {
        dec @x = @y;
    }
}

fn main() -> i32 {
    assign_val!(foo = 42);
    return foo;
}
