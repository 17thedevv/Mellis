module std {
    export enum Option<T> { None, Some(T) }
}
fn main() {
    dec val: i32 = 42;
    dec opt: std::Option<&i32> = std::Option::Some<&i32>(&val);
}
