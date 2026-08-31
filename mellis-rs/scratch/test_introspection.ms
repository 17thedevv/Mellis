fn main() -> i32 {
    dec size: i32 = sizeof(i32);
    dec align: i32 = alignof(i32);
    
    if size == 4 {
        if align == 8 {
            return 0;
        }
        return 2;
    }
    
    return 1;
}

module std {
    module abc {
        fn add() {

        }
    }
}
