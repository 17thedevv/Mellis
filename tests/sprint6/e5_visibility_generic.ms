mod m {
    export struct View<T> {
        data: T;
    }

    export fn create_view<T>(val: T) -> View<T> {
        return View { data: val };
    }
}

fn main() {
    dec v = m::create_view(5);
    // Should error because `data` is private and we are outside module `m`
    dec x = v.data; 
}
