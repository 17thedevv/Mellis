struct List {
    data: i32,
}

impl List {
    fn get(self: &List, i: u64) -> &i32 {
        return &self.data;
    }
}

fn main() {
    dec rw list = List { data: 42 };
    dec x = list.get(0);
    list.data = 100; // should fail because list is borrowed
    dec y = x;
}
