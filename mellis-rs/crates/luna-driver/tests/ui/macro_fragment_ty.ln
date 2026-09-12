macro declare_var {
    (@name: ident, @t: ty, @val: expr) => {
        dec @name: @t = @val;
    }
}

fn main() -> i32 {
    declare_var!(answer, i32, 42);
    return answer;
}
