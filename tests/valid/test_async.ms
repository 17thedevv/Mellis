// ==========================================
// test_async.ms — comprehensive async/await semantics
// ==========================================

// Test 1: Basic async fn with return value — generates Future<int_32>
async fn fetch_number() -> int_32 {
    return 42;
}

// Test 2: async fn that chains with .await
async fn double_async(x: int_32) -> int_32 {
    dec base = fetch_number().await;
    return base + x;
}

// Test 3: async fn with void return
async fn do_work() {
    // Async function with no explicit return value
}

// Test 4: Multiple awaits in sequence
async fn sequential() -> int_32 {
    dec a = fetch_number().await;
    dec b = fetch_number().await;
    return a + b;
}

fn main() -> int_32 {
    return 0;
}
