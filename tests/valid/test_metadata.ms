use metadata_mod;
export struct MyStruct {}
impl metadata_mod::MyTrait for MyStruct {
    fn trait_func(self: *rw Self) -> uint_64 { return 42 as uint_64; }
}
fn main() {
    dec s = MyStruct {};
    metadata_mod::uses_trait@<MyStruct>(&rw s as *rw MyStruct);
}
