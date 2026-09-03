struct Counter {
    value: int_32;
}

impl Counter {
    fn read(self: &Counter) -> int_32 {
        return self.value;
    }

    fn inc(self: &rw Counter) {
        self.value = self.value + 1;
    }
}

fn main() {
    dec rw c = Counter { value: 0 };
    
    dec r = &c;
    r.read(); // shared borrow
    
    // We cannot mutably borrow c while r is live!
    dec rw mut_c = &rw c;
    mut_c.inc(); // mutable borrow
    
    // r is used here
    dec x = r.read();
}
