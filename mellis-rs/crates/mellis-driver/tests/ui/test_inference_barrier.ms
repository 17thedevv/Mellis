fn make_value<T>() -> T {
    while true {}
}

fn main() {
    dec x = make_value(); //~ ERROR cannot infer type for expression
}

