// ==========================================
// Mellis Language Showcase
// ==========================================

// 1. Modules and Imports
use core_lib::{ID, Status, Processor};
use math::vector;

// 2. Constants & Statics
const VERSION: int_32 = 1;
// Note: FDLang doesn't have 'static' keyword natively yet, using const

// 3. Type Aliases
type NodeID = uint_32;

// 4. Structs & Generics
export struct Task<T> {
    id: ID;
    data: T;
    is_done: bool;
}

// 5. Traits & Implementations
impl<T> Processor for Task<T> {
    fn process(self) -> int_32 {
        return 1;
    }
}

// Struct implementation with methods
impl<T> Task<T> {
    fn mark_done(self) {
        // Method mutating self
    }
}

// 6. Functions & Pattern Matching
fn process_status(state: Status<int_32>) -> int_32 {
    dec result_code = 0;
    
    // Pattern matching
    match state {
        Status::Success(val) -> {
            result_code = val;
        },
        Status::Failure -> {
            result_code = -1;
        }
    };
    
    return result_code;
}

fn calculate(factor: int_32, t: &Task<str>) -> int_32 {
    return factor * 2;
}

// 7. Main entry point
export fn main() -> int_32 {
    // Variable declaration and Struct instantiation
    dec my_id: ID = 100u32;
    dec task = Task@<str> {
        id: my_id,
        data: "Mellis Showcase",
        is_done: false
    };

    // Borrowing & References
    dec ref_task = &task;
    dec cal_val = calculate(5, ref_task);
    
    // Method & Trait calls
    task.mark_done();
    task.process();

    // Enums
    dec state = Status::Success(1);
    dec status_val = process_status(state);

    // Arrays
    dec arr: [int_32; 3] = [10, 20, 30];
    dec first = arr[0];

    // Closures
    dec captured = 50;
    dec closure = |x: int_32| -> int_32 {
        return x + captured;
    };
    dec closure_res = closure(10); // 60

    // Unsafe & Pointers
    dec raw_val: int_32 = 100;
    unsafe {
        dec ptr: *rw int_32 = &raw_val as *rw int_32;
        dec next_ptr = ptr + 1; // Pointer arithmetic
        *ptr = 200;             // Dereference
    }
    
    // Using imported modules
    dec vec1 = vector::Vector2 { x: 1, y: 2 };
    dec vec2 = vector::Vector2 { x: 3, y: 4 };
    dec vec3 = vector::add(vec1, vec2);
    
    return status_val;
}
