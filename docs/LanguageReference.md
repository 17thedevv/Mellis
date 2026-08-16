# Ngôn ngữ Mellis (FDLang) - Tài liệu tham khảo (Language Reference)

Mellis (hay còn gọi là FDLang) là một ngôn ngữ lập trình hệ thống được thiết kế hướng tới sự an toàn, hiệu năng cao và cú pháp hiện đại. Ngôn ngữ có mượn nhiều ý tưởng từ Rust và C++ nhưng được tinh chỉnh để có một trình biên dịch nhanh hơn, nhẹ hơn và dễ dự đoán hơn.

Dưới đây là tài liệu chi tiết về tất cả các tính năng (features) hiện tại của ngôn ngữ Mellis.

---

## 1. Kiểu dữ liệu cơ bản (Primitive Types)
Mellis hỗ trợ các kiểu dữ liệu cơ bản sau:
- **Số nguyên:** `int_8`, `int_16`, `int_32`, `int_64`
- **Số nguyên không dấu:** `uint_8`, `uint_16`, `uint_32`, `uint_64`
- **Số thực:** `float_32`, `float_64`
- **Boolean:** `bool` (nhận giá trị `true` hoặc `false`)
- **Void:** `void` (dành cho hàm không trả về giá trị)
- **Chuỗi:** `string` (chuỗi ký tự UTF-8)

## 2. Biến và Tính Đột Biến (Variables & Mutability)
Mặc định, tất cả các biến trong Mellis đều là **immutable** (không thể thay đổi giá trị sau khi khởi tạo). Để cho phép thay đổi, bạn phải sử dụng từ khóa `mut`. Khai báo biến bắt đầu bằng từ khóa `dec`.

```rust
dec x: int_32 = 10; // Immutable
// x = 20; // Lỗi biên dịch!

dec mut y: int_32 = 10; // Mutable
y = 20; // Hợp lệ
```

Mellis hỗ trợ **Type Inference** (tự động suy luận kiểu). Nếu có giá trị khởi tạo, bạn có thể bỏ qua khai báo kiểu dữ liệu:
```rust
dec a = 42;       // Tự động suy luận là int_32
dec mut b = true; // Tự động suy luận là bool
```

## 3. Điều khiển luồng (Control Flow)
### Cấu trúc rẽ nhánh `if / else`
Mellis hỗ trợ `if / else if / else` với cú pháp biểu thức khối.
```rust
if x > 10 {
    print!("Lớn hơn 10");
} else if x == 10 {
    print!("Bằng 10");
} else {
    print!("Nhỏ hơn 10");
}
```

### Vòng lặp `while`
Vòng lặp `while` cho phép lặp khi điều kiện vẫn là `true`.
```rust
dec mut count = 0;
while count < 5 {
    count = count + 1;
}
```

## 4. Hàm (Functions)
Hàm được định nghĩa bằng từ khóa `fn`. Bạn có thể trả về giá trị bằng từ khóa `return` hoặc sử dụng **Tail Expression** (biểu thức cuối không có dấu chấm phẩy) để trả về tự động.

```rust
fn add(a: int_32, b: int_32) -> int_32 {
    a + b // Tail expression, ngầm định trả về a + b
}

fn do_something() -> void {
    print!("Hello!");
    // Ngầm định trả về void
}
```

## 5. Structs (Cấu trúc dữ liệu)
Mellis hỗ trợ ba loại Struct: Named Struct, Tuple Struct và Unit Struct.

```rust
// 1. Named Struct
struct Point {
    x: int_32,
    y: int_32,
}
dec p = Point { x: 10, y: 20 };

// 2. Tuple Struct
struct Color(int_32, int_32, int_32);
dec c = Color(255, 0, 0);

// 3. Unit Struct (Không chứa dữ liệu)
struct Empty;
dec e = Empty;
```

## 6. Enums và Pattern Matching (Đại số dữ liệu)
Enum trong Mellis mạnh mẽ, có thể lưu trữ dữ liệu (Algebraic Data Types). Kết hợp với từ khóa `match`, Mellis cung cấp Pattern Matching siêu việt, đảm bảo **tính toàn vẹn (exhaustiveness)**.

```rust
enum Result<T, E> {
    Ok(T),
    Err(E)
}

fn handle(res: Result<int_32, int_32>) -> int_32 {
    match res {
        Result::Ok(val) => { val * 2 },
        Result::Err(err) => { err * -1 }
    }
}
```
Pattern matching hỗ trợ nhiều loại pattern:
- **Identifier Pattern:** `val`, `err`
- **Literal Pattern:** `1`, `"hello"`
- **Wildcard Pattern:** `_` (khớp mọi giá trị)
- **Nested Pattern:** `Result::Ok(Some(x))`

## 7. Generics (Lập trình tổng quát) & Monomorphization
Mellis cho phép định nghĩa các hàm, Struct và Enum có tính tổng quát bằng cú pháp `<T>`.
Mellis sử dụng cơ chế **Monomorphization** để sinh ra mã tối ưu (Zero-cost abstractions) riêng cho từng kiểu dữ liệu được sử dụng.

```rust
struct Container<T> {
    item: T
}

fn get_item<T>(c: Container<T>) -> T {
    c.item
}
```

## 8. Traits & Interface
Traits định nghĩa các hành vi chung (interface) mà nhiều kiểu dữ liệu có thể thực thi (`impl`). Mellis cũng hỗ trợ **Generic Trait Bounds** (`T: Trait`).

```rust
trait Process {
    fn process(self) -> int_32;
}

struct Data { val: int_32 }

// Cài đặt Trait cho Struct
impl Process for Data {
    fn process(self) -> int_32 {
        self.val
    }
}

// Yêu cầu kiểu T phải tuân thủ Trait Process
fn run_process<T: Process>(item: T) -> int_32 {
    item.process()
}
```

## 9. Tham chiếu và Mượn bộ nhớ (Borrowing & Memory Safety)
Mellis tích hợp cơ chế **Borrow Checker** đảm bảo bộ nhớ được truy cập an toàn mà không cần Garbage Collector.
- Tham chiếu mượn chỉ đọc: `&T`
- Tham chiếu mượn ghi: `&mut T`

```rust
fn modify(val: &mut int_32) {
    // thao tác trực tiếp trên biến gốc
}

dec mut x = 10;
modify(&mut x);
```
Trình biên dịch sẽ ngăn chặn các truy cập lỗi (ví dụ: mượn `&mut` khi đã có một tham chiếu `&` khác tồn tại cùng thời điểm, hoặc hàm generic không vượt qua ràng buộc borrow checking).

## 10. Type Aliasing
Cho phép đặt tên khác (bí danh) cho các kiểu dữ liệu phức tạp. Có thể hỗ trợ cả Generic Alias.

```rust
type UserId = uint_64;
type Pair<T> = (T, T); // Tuple alias

dec id: UserId = 100;
```

## 11. Closures (Hàm nặc danh / Lambda)
Closure (`|| { }`) trong Mellis hỗ trợ thu thập (capture) các biến môi trường từ scope bên ngoài. Mellis tự động cấp phát Heap cho closure và sử dụng semantics của `Drop` để tự động giải phóng môi trường khi Closure kết thúc vòng đời.

```rust
dec factor = 2;
dec closure = |x: int_32| -> int_32 {
    x * factor
};
```

## 12. Macros (Siêu lập trình)
FDLang / Mellis sở hữu các builtin macros được gọi thông qua dấu chấm than `!`. Các macros này được xử lý ngay trong quá trình dịch (AST Expansion).
- `print!("Hello {}!", name)`: In ra màn hình.
- `println!("...")`: In ra màn hình và xuống dòng.
- `assert!(condition)`: Báo lỗi và dừng chương trình (panic) nếu condition `false`.
- `vec![1, 2, 3]`: Cú pháp tiện ích cho cấp phát mảng.

## 13. Hệ thống Module (`.mlib`)
Mellis thiết kế một hệ thống nạp và chia sẻ thư viện tiên tiến có đuôi file là `.mlib`.
Bạn có thể biên dịch một tệp `.ms` thành tệp thư viện `.mlib` để tái sử dụng nhanh chóng.

```rust
// Cú pháp xuất (export) tính năng ra khỏi Module
export fn util() {}
```

Bên file consumer sử dụng cú pháp `use` để nhập từ thư viện:
```rust
use my_library::{util, MyStruct};

fn main() -> void {
    util();
}
```
*Ghi chú:* Generic functions và Traits từ thư viện `.mlib` hoàn toàn có thể được nạp và chuyên biệt hóa (Monomorphize) tại file Consumer.

## Tổng kết
Mellis là sự kết hợp hoàn hảo giữa cú pháp mềm dẻo, pattern matching siêu việt, hệ thống type-safe nghiêm ngặt thông qua Borrow Checker / Generic Monomorphization và hiệu năng tối đa của LLVM Backend. Ngôn ngữ đảm nhận trách nhiệm ngăn lỗi Runtime từ giai đoạn biên dịch một cách chủ động (Compile-time Verification).
