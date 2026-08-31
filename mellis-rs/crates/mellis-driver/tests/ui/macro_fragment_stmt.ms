macro run_stmt {
    (@s: stmt) => {
        @s
    }
}

fn main() -> i32 {
    dec rw count: i32 = 0;
    run_stmt!(count = count + 10;);
    return count;
}
