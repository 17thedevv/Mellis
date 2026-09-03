fn foo(x: &i32) -> &i32 life_from(x) {
    return x;
}

fn bar(a: &i32, b: &i32) -> &i32 
    life_from(a | b)
    where outlives(a, b)
{
    return a;
}
