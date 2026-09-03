
fn print_side_effect() -> bool {
    // We would print something here if we had print
    return true;
}

fn main() -> i32 {
    dec x = false;
    dec y = x && print_side_effect(); // Should short circuit
    
    dec a = true;
    dec b = a || print_side_effect(); // Should short circuit
    
    return 0;
}
