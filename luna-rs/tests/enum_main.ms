import "enum_lib";

fn main() {
    dec opt: std::Option<i32> = std::Option::Some<i32>(42);
    dec val = std::get_or_default<i32>(opt, 0);
}
