import "mock";

fn main() -> i32 {
    // mock imports core, which defines ControlFlow.
    // However, mock does NOT explicitly re-export ControlFlow.
    // If the module registry correctly filters non-local symbols, 
    // ControlFlow should NOT be visible here.
    dec x: ControlFlow<i32, i32>; //~ ERROR Symbol 'ControlFlow' not found
    return 0;
}
