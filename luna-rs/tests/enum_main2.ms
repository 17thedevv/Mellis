import "enum_lib";

fn main() {
    dec opt: std::Option<i32> = std::Option::Some<i32>(42);
    dec result: i32 = 0;
    match opt {
        std::Option::Some(val) -> {
            result = val;
        }
        std::Option::None -> {
            result = -1;
        }
    }
}
