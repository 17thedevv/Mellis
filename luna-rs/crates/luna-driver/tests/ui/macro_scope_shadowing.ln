macro compute {
    (@x: expr) => {
        @x + 1
    }
}

module inner {
    export macro compute {
        (@x: expr) => {
            @x + 100
        }
    }
}

fn main() -> i32 {
    dec a: i32 = compute!(10);
    dec b: i32 = inner::compute!(10);
    return a + b;
}
