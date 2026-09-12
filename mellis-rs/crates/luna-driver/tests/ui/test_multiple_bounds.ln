trait Debug {
    fn print(self: &rw Self);
}

trait Clone {
    fn clone(self: &rw Self) -> Self;
}

struct DoubleTraitStruct {}
impl Debug for DoubleTraitStruct {
    fn print(self: &rw DoubleTraitStruct) {}
}
impl Clone for DoubleTraitStruct {
    fn clone(self: &rw DoubleTraitStruct) -> DoubleTraitStruct {
        return DoubleTraitStruct {};
    }
}

struct SingleTraitStruct {}
impl Debug for SingleTraitStruct {
    fn print(self: &rw SingleTraitStruct) {}
}

fn process<T: Debug + Clone>(item: &rw T) {
    item.print();
}

fn main() {
    dec rw good = DoubleTraitStruct {};
    dec rw bad = SingleTraitStruct {};

    process(&rw good); // OK
    process(&rw bad); //~ ERROR The type `SingleTraitStruct` does not implement trait `Clone` (required by inferred generic parameter `T`)
}
