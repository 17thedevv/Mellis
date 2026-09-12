struct Counter {
    val: int_32;
}

impl Counter {
    fn new() -> Self {
        return Counter { val: 0 };
    }

    fn inc(&mut self) {
        self.val = self.val + 1;
    }

    fn get(&self) -> int_32 {
        return self.val;
    }
}

fn main() -> int_32 {
    dec c = Counter { val: 0 };
    c.inc();
    return c.get();
}
