struct MyStruct {
    val: i32
}

trait Drop {
    fn drop(self: &rw Self);
}


impl Drop for MyStruct {
    fn drop(self: &rw Self) {}
}

fn do_something(x: MyStruct) {}

fn main() {
    dec s = MyStruct { val: 1 }; //~ ERROR Cannot drop conditionally moved value
    
    while true {
        do_something(s); // Moves `s`
        break; // Emits Drop for `s` because it leaves scope, but should be stripped by MoveAnalyzer!
    }
}
