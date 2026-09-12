fn add(a: int, b: int) -> int {
    return a + b;
}

fn apply_op(op: fn(int, int) -> int, a: int, b: int) -> int {
    return op(a, b);
}

fn get_op() -> fn(int, int) -> int {
    return add;
}

fn main() -> int {
    dec rw my_add: fn(int, int) -> int = add;
    dec res1 = my_add(10, 20);
    
    dec res2 = apply_op(my_add, 5, 5);
    
    dec returned_op = get_op();
    dec res3 = returned_op(2, 3);
    
    if res1 != 30 { return 1; }
    if res2 != 10 { return 2; }
    if res3 != 5 { return 3; }
    
    return 0;
}
