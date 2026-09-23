// Comptime array length calculation test //~ ERROR type mismatch

const LEN = 2 + 2; //~ ERROR cannot evaluate constant in comptime

fn main() -> i32 {
    dec arr: [i32; LEN] = [1, 2, 3, 4]; //~ ERROR cannot evaluate array size in comptime
    arr[0] + arr[1] + arr[2] + arr[3]
}
