// test_tuple_index.ms
// Tests for Tuple Indexing semantics.

fn main() -> i32 {
    let t = (10, 20);
    let x = t.0; // Happy path
    let y = t.1;

    let nested = ((1, 2), 3);
    let z = nested.0.1; // Nested path

    return x + y + z; // Should be 10 + 20 + 2 = 32
}

fn test_errors() {
    let t = (1, 2);
    // t.2;        // Expected error: out of bounds
    // t.-1;       // Parser should reject or emit invalid index error (uint32_t parses as huge number -> out of bounds)
    
    let x = 5;
    // x.0;        // Expected error: non-tuple
}
