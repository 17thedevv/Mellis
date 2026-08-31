fn main() -> i32 {
    dec res: i32 = add_ten!(15);
    return res;
}

macro add_ten {
    (@x: expr) => {
        @x + 10
    }
}
