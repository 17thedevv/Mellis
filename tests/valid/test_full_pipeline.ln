enum Option<T> {
    Some(T),
    None,
}

trait Display {
    fn format(self) -> *const u8;
}

impl Display for int_32 {
    fn format(self) -> *const u8 {
        return "number";
    }
}

fn print_opt<T: Display>(opt: Option<T>) {
    match opt {
        Option::Some(val) -> {
            dec msg = val.format();
        },
        Option::None -> {
        }
    }
}

fn main() -> int_32 {
    dec a = Option::Some(42);
    dec b: Option<int_32> = Option::None;
    
    print_opt(a);
    print_opt(b);
    
    return 0;
}
