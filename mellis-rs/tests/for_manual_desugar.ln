// Standalone for-loop test (no imports)
// Tests the core for-loop desugaring with a hand-rolled iterator

enum Option {
    Some(i32),
    None,
}

struct Counter {
    current: i32;
    max: i32;
}

fn counter_new(max: i32) -> Counter {
    return Counter { current: 0, max: max };
}

impl Counter {
    fn next(self: &rw Self) -> Option {
        if self.current < self.max {
            dec val = self.current;
            self.current = self.current + 1;
            return Option::Some(val);
        }
        return Option::None;
    }
}

fn main() -> i32 {
    dec rw c = counter_new(5);

    dec rw sum: i32 = 0;
    // Manual desugared loop (what for-loop becomes)
    while true {
        match c.next() {
            Option::Some(item) -> {
                sum = sum + item;
            },
            Option::None -> {
                break;
            },
        }
    }

    // Expected: 0 + 1 + 2 + 3 + 4 = 10
    return sum;
}
