
fn foo() {}
fn bar(x: i32) {}

fn main() {
    foo()?; //~ ERROR The `?` operator can only be applied to types that implement `Try`
            //~ ERROR The `?` operator cannot be used in a function that returns this type, as it does not implement `FromResidual` for the residual type
    dec x = bar(0)?; //~ ERROR The `?` operator can only be applied to types that implement `Try`
    foo(x)?; //~ ERROR The `?` operator can only be applied to types that implement `Try`
}
