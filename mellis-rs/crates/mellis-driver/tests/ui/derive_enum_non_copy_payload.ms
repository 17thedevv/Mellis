#[derive(Clone)]
struct BoxedInt {
    val: i32,
}

#[derive(Clone)]
enum OptionBoxed {
    Some(BoxedInt),
    None,
}

fn main() -> i32 {
    dec original = OptionBoxed::Some(BoxedInt { val: 42 });
    dec cloned = original.clone();
    
    match cloned {
        OptionBoxed::Some(b) -> { return b.val; }
        OptionBoxed::None -> { return 0; }
    }
}
