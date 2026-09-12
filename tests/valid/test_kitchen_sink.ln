// test_kitchen_sink.ms
// Thử nghiệm toàn bộ các tính năng cơ bản của Mellis

// 1. Enums và Generics
enum Option<T> {
    Some(T),
    None
}

// 2. Trait (Interface)
trait Computable {
    fn compute(self) -> int_32;
}

// 3. Struct
struct Data {
    value: int_32;
}

// 4. Impl Trait cho Struct
impl Computable for Data {
    fn compute(self) -> int_32 {
        return self.value * 2;
    }
}

// 5. Hàm Generic với Trait Bound và Pattern Matching
fn process_option<T: Computable>(opt: Option<T>) -> int_32 {
    match opt {
        Option::Some(val) -> {
            // Biến val được ràng buộc từ pattern match
            return val.compute();
        },
        Option::None -> {
            return 0;
        }
    }
}

fn main() -> int_32 {
    // 6. Khai báo biến (Immutable và Mutable)
    dec data = Data { value: 10 };
    dec sum = 0;
    
    // 7. Gọi hàm Generic (Monomorphization sẽ được kích hoạt ở đây)
    dec opt = Option::Some(data);
    dec result = process_option(opt);
    
    // 8. Control Flow (If / Else)
    if result > 10 {
        sum = sum + result;
    } else {
        sum = sum - result;
    }
    
    // 9. Closures (Lambda captures environment)
    dec factor = 5;
    dec cl = |x: int_32| -> int_32 {
        return x + factor + sum;
    };
    
    dec final_val = cl(result);
    
    return final_val;
}
