enum Color {
    Red,
    Green,
    Blue,
}

fn main() {
    dec c = Color::Red;
    match c { //~ ERROR Match is not exhaustive. Covered 2/3 enum variants.
        Color::Red -> {},
        Color::Green -> {},
    }
}
