import "mock";

fn leak_test() {
    dec x = branch; //~ ERROR Symbol 'branch' not found
}

fn main() -> i32 {
    return 0;
}
