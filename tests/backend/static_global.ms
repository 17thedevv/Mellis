dec GLOBAL_COUNT: int_32 = 0;

fn main() -> int_32 {
    GLOBAL_COUNT = GLOBAL_COUNT + 1;
    return GLOBAL_COUNT;
}
