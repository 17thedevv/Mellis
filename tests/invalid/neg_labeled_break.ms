// neg_labeled_break.ms
// EXPECT_ERROR: Use of undeclared label ''outer'

fn main() -> i32 {
    'inner: while (true) {
        break 'outer;
    }
    return 0;
}
