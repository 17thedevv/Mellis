trait A {}
trait B {}

struct DummyA {}
impl A for DummyA {}

struct DummyAB {}
impl A for DummyAB {}
impl B for DummyAB {}

fn require_both<T: A + B>(x: T) {}

fn main() {
    dec d1 = DummyA {};
    require_both(d1); //~ ERROR The type `DummyA` does not implement trait `B` (required by inferred generic parameter `T`)

    dec d2 = DummyAB {};
    require_both(d2); // Should succeed
}
