struct NonCopy { val: i32 }
    
export fn main() -> i32 {
    dec x = NonCopy { val: 42 };
    dec f = |z: i32| -> i32 {
        return x.val + z;
    };
    // using x again should fail because it was moved into the closure
    dec y = x.val;
    return y;
}
