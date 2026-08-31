fn main() -> i32 {
    dec x: i32 = unknown_macro!(10); //~ ERROR no macro named `unknown_macro` in scope
    return x;
}
