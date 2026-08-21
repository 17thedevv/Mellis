trait BadTrait1 {
    fn bad_method(self) -> int_32;
}

trait BadTrait2 {
    fn bad_method<T>(&self, x: T) -> int_32;
}

trait BadTrait3 {
    fn bad_method(&self) -> Self;
}

struct Dummy {
    val: int_32;
}

impl BadTrait1 for Dummy {
    fn bad_method(self) -> int_32 {
        return self.val;
    }
}

impl BadTrait2 for Dummy {
    fn bad_method<T>(&self, x: T) -> int_32 {
        return self.val;
    }
}
xz/.,mnb
impl BadTrait3 for Dummy {
    fn bad_method(&self) -> Self {
        return *self;
    }
}

fn main() -> int_32 {
    dec d = Dummy { val: 42 };
    
    // All 3 of these should fail with E-OBJECT-SAFE!
    dec ptr1: &dyn BadTrait1 = &d; 
    dec ptr2: &dyn BadTrait2 = &d; 
    dec ptr3: &dyn BadTrait3 = &d; 
    
    return 0;
}
