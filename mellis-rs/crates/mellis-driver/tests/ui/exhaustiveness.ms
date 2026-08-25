enum Color {
    Red,
    Green,
    Blue,
}

fn main() {
    dec c = Color::Red;
    match c {
        Color::Red -> {},
        Color::Green -> {},
        //~ ERROR Match is not exhaustive
    }
}
