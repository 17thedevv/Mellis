macro wrap {
    (@b: block) => {
        { @b }
    }
}

fn main() -> i32 {
    dec rw x: i32 = 10;
    wrap!({
        x = x + 5;
    });
    return x;
}
