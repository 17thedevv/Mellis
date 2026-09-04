fn foo() {}
fn bar(x: i32) {}

fn main() {
    foo()?; //~ ERROR the `?` operator can only be applied to values that implement `Try`
    dec x = bar(0)?; //~ ERROR the `?` operator can only be applied to values that implement `Try`
    foo(x)?; //~ ERROR the `?` operator can only be applied to values that implement `Try`
}
