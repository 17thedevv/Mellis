// typeof as an expression — negative test

fn main() {
    dec x = typeof(42); //~ ERROR 'typeof' is a type-level query and cannot be used as an expression
} //~ ERROR Expected expression.
