// test_tuple_index.ms
// Tests for Tuple Indexing semantics.

fn main() -> int_32 {
    dec t = (10, 20);
    dec x = t.0; // Happy path
    dec y = t.1;

    dec nested = ((1, 2), 3);
    dec z = nested.0.1; // Nested path

    return x + y + z; // Should be 10 + 20 + 2 = 32
}

fn test_errors() {
    dec t = (1, 2);
    // t.2;        // Expected error: out of bounds
    // t.-1;       // Parser should reject or emit invalid index error (uint32_t parses as huge number -> out of bounds)
    
    dec x = 5;
    // x.0;        // Expected error: non-tuple
}
