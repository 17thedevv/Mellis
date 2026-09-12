// Positive test: generic function calls in comptime

fn identity<T>(x: T) -> T {
    x
}

struct Point<T> {
    x: T,
    y: T,
}

fn calculate_area<T>(p: Point<T>) -> T {
    p.x * p.y
}

const CONST_IDENT: i32 = comptime {
    identity(42)
};

const CONST_AREA: i32 = comptime {
    dec p = Point<i32> { x: 5, y: 10 };
    calculate_area(p)
};

fn main() -> i32 {
    CONST_IDENT + CONST_AREA // 42 + 50 = 92
}
