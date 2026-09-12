fn foo() {}
fn foo() {}

fn main() {
    dec x = 1;
    dec x = 2;

    {
        dec y = 3;
        dec y = 4;
    }

    {
        dec z = 5;
    }
    dec z = 6; // Not shadowing, distinct scopes
    
    dec outer = 1;
    {
        dec outer = 2; // Shadowing outer scope, should be valid!
    }
}
