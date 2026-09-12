import "trait_lib";

fn print_it<T: trait_lib::Printable>(item: &T) {
    item.print();
}

fn main() {
    dec p = trait_lib::Point { x: 1, y: 2 };
    print_it<trait_lib::Point>(&p);
}