// =============================================================================
// Test: Deep Visibility Leakage
// Description: Verifies that public APIs cannot leak private types through
//              fields, function signatures, generic parameters, or associated types.
// =============================================================================

struct PrivateStruct {
    x: int_32;
}

export struct PublicStruct {
    // Should fail: public struct with a private field type exposed
    pub_field: PrivateStruct; // Error: private type in public struct
}

export fn expose_private() -> PrivateStruct { // Error: public function returning private type
    return PrivateStruct { x: 10 };
}

export trait PubTrait {
    fn do_something(val: PrivateStruct); // Error: private type in public trait method
}

export fn main() -> int_32 {
    return 0;
}
