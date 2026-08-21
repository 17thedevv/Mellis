use core;
use core::option::Option;
use alloc::vec::{Vec, vec_new};

fn main(args: [str]) -> int_32 {
    dec v: Vec<int_32> = vec_new@<int_32>();
    v.push(42);
    dec item = v.pop();
    return item.unwrap_or(0);
}
