struct NonCopy { val: i32 }

export fn main() -> i32 {
    dec x = NonCopy { val: 42 };
    dec f = move |z: i32| -> i32 {
        return x.val + z;
    };
    dec y = x.val; //~ ERROR Use of moved value
    return y;
}
