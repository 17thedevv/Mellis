mod secret {
    export struct SecretKey {
        value: int_32;
    }
    
    export impl SecretKey {
        fn get_value(self: SecretKey) -> int_32 {
            return self.value;
        }
    }
    
    export fn new_key() -> SecretKey {
        return SecretKey { value: 42 };
    }
}

fn main() {
    dec key = secret::new_key(); // OK: new_key is public
    dec val = key.get_value();          // Error: SecretKey::get_value is private
    dec inner = key.value;              // Error: field value is private
}
