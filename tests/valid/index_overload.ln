trait Index<Idx> {
    fn index(self, idx: Idx) -> int_32;
}

struct Array {
    data: int_32;
}

impl Index<int_32> for Array {
    fn index(self, idx: int_32) -> int_32 {
        return self.data + idx;
    }
}

fn main() -> int_32 {
    dec arr = Array { data: 100 };
    dec val = arr[5];
    return val;
}
