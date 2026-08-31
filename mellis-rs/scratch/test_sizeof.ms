struct LargeStruct {
    a: i32,
    b: i32,
    c: i32,
}

fn main() -> i32 {
    dec s1 = sizeof(i32);
    dec a1 = alignof(i32);
    
    dec s2 = sizeof(LargeStruct);
    dec a2 = alignof(LargeStruct);
    
    return s1 + a1 + s2 + a2;
}
