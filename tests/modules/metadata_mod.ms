export trait MyTrait {
    fn trait_func(self: *rw Self) -> uint_64;
}
export fn uses_trait<T: MyTrait>(val: *rw T) -> uint_64 {
    return val.trait_func();
}
