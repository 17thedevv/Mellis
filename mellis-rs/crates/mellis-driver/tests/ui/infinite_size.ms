struct A { //~ ERROR Recursive type has infinite size
    a: A
}

fn main() {
    dec x = 1;
}
