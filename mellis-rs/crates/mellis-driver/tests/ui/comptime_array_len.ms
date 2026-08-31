// Comptime array length calculation test

const LEN: usize = 2 + 2;

fn main() -> i32 {
    dec arr: [i32; LEN] = [1, 2, 3, 4];
    arr[0] + arr[1] + arr[2] + arr[3]
}
