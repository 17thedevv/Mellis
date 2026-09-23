fn make_adder(x: i32) -> fn(i32) -> i32 {
    return |y: i32| -> i32 {
        return x + y;
    };
}

fn main() -> i32 {
    dec add5 = make_adder(5);
    dec res1 = add5(10);
    
    dec add20 = make_adder(20);
    dec res2 = add20(10);
    
    if res1 != 15 { return 1; }
    if res2 != 30 { return 2; }
    
    return 0;
}
