# Kế hoạch đưa FD-Lang lên Production & Hoàn thiện Hệ thống Module

## 1. Các cú pháp hiện tại CHƯA biên dịch được trong `my_app`
Dựa trên ví dụ đồ sộ bạn vừa cung cấp, ngôn ngữ của chúng ta **mới chỉ dịch được khoảng 10-15%** các tính năng thực sự (chủ yếu là basic variables, function, và vòng lặp/điều kiện). Dưới đây là danh sách những thứ **CHƯA được dịch** (từ parser, resolver, typechecker cho đến MVIR):

1. **Hệ thống Module & Use (`mod`, `use`)**: Parser đã có node, nhưng `Resolver` không biết import từ file khác, `TypeChecker` không xử lý scope module.
2. **Tuples, Enums & Pattern Matching (`match`)**: Mới chỉ nằm trên giấy (grammar). AST và MVIR hoàn toàn trống trơn phần này. Cả Enum destructuring và Tuple pattern matching đều chưa có.
3. **Traits, Generics (`impl<T>`, `trait`) & dyn**: Là phần phức tạp bậc nhất (Monomorphization/VTable), hiện tại chưa đụng đến.
4. **Structs, Struct Literals & Named args**: Mới chỉ parse được cái vỏ AST, chưa cấp phát và truy xuất (`.`) trên MVIR. Phương thức (`self`) cũng chưa được xử lý.
5. **Casting (`as`), Sizeof, Alignof**: Chưa emit được LLVM IR tương ứng.
6. **Async/Await, Never Type (`!`), Pointers (`*`, `&rw`), Unsafe, Comptime**: Hoàn toàn chưa có.
7. **Nhiều kiểu Literals**: Suffix (`42i32`), Float, Unicode escapes, Unit `()`, Byte literal.
8. **Các toán tử Unary/Postfix (`++`, `--`) và Range (`0..10`)**: Chưa sinh được MVIR.

---

## 2. Lộ trình "Road to Production" (Kế hoạch tổng thể)

Để hoàn thành toàn bộ những tính năng này, chúng ta cần chia thành các Phase lớn:

- **Phase 5: Hệ thống Module & Project Structure (Mục tiêu hiện tại)**: `mod`, `use`, Import resolution, Visibility (`export`).
- **Phase 6: Data Structures (Structs & Tuples)**: Định nghĩa Struct, Tuple, khởi tạo bằng Literals, truy xuất các biến thành viên (`.`, index).
- **Phase 7: Array & Indexing**: Array literal (`[1, 2, 3]`), truy xuất mảng bằng `[index]`, hoàn thiện `for x in arr`.
- **Phase 8: Pointer, Tham chiếu & Mượn (Borrow Checker V1)**: `&`, `&rw`, `*ptr`, `unsafe`. Cấu hình cơ bản cho mượn bộ nhớ.
- **Phase 9: Enum & Pattern Matching (`match`)**: Variant, Destructuring, Exhautiveness checking.
- **Phase 10: Traits & Generics (Cơ bản)**: Generic structs/functions, Inherent `impl`, Trait implementation tĩnh (Monomorphization).
- **Phase 11: Dynamic Dispatch (`dyn`)**: VTable generation.
- **Phase 12: Async / Await & Advanced (Casting, Macros/Comptime)**.

---

## 3. Kế hoạch triển khai Phase 5: Hoàn thiện hệ thống Module (Theo yêu cầu)

Bạn yêu cầu làm hệ thống Module nhưng không cung cấp Standard Lib để bạn tự viết. Kế hoạch như sau:

### Phân tích vấn đề
Hiện tại trình biên dịch chỉ xử lý được **một file duy nhất** (qua hàm `main`). `Resolver` và `TypeChecker` làm việc trên 1 AST chung. Để hỗ trợ `mod` và `use`, ta cần một **Trình quản lý Modules (Module/Package Manager)** nằm ở lõi của Compiler.

### Đề xuất thiết kế (Kiến trúc Compiler)

1. **Khái niệm `CompilationUnit` (hay `ModuleContext`)**: 
   Thay vì parse thẳng 1 file `test.ms` ra 1 `AST` và quăng vào `Resolver`, ta sẽ tạo một `Workspace` (hoặc `CompilerSession`), nó sẽ chứa danh sách các `Module`. Mỗi module đại diện cho 1 file (hoặc thư mục).
   
2. **`import/use` Resolution**: 
   - Thêm bước **`Module Resolution Phase`** trước khi chạy `Resolver` tên biến.
   - Khi parser thấy `use std::io`, nó sẽ đi tìm `std/io.ms` hoặc thư mục `std/io/mod.ms`.
   - Biên dịch file đó thành AST phụ, nạp vào bảng Global Module Table.

3. **Cập nhật `Resolver`**: 
   - Bảng symbol (SymbolTable) hiện tại là 1 cây dẹt. Ta cần đưa biến toàn cục/hàm của module A vào 1 Namespace (Scope) của module A.
   - Khi module B gọi `use A`, `Resolver` của module B sẽ có thể look-up các symbol trong module A nếu chúng được đánh dấu `export` (public).

4. **Biên dịch (LLVM / MVIR)**: 
   - Tất cả các module AST sẽ được dồn vào chung 1 `MVIRGenerator`, hoặc sinh ra nhiều Module LLVM và `lld-link` chúng lại với nhau ở bước cuối cùng. Để đơn giản bước đầu, ta sẽ gộp tất cả AST của các module được `use` vào chung một MVIRContext lớn.

> [!IMPORTANT]
> **Quyết định thiết kế**: Khi gặp câu lệnh `use a::b::c;`, trình biên dịch sẽ tìm kiếm file ở đường dẫn `./a/b/c.ms` so với thư mục gốc của project (có thể được định nghĩa bằng thư mục chứa file `main.ms`). Bạn có đồng ý với cơ chế map trực tiếp Tên Module = Tên File/Thư mục này không?

> [!NOTE]
> Về việc bỏ `print` keyword và dùng `extern fn printf`: Cú pháp `extern fn` đã có trong parser. Tôi sẽ xóa keyword `print` khỏi Lexer/Parser để biến nó thành một lời gọi hàm `CallExpr` bình thường, giúp bạn hoàn toàn tự do xây dựng thư viện.

Bạn có đồng ý với Roadmap này và cách thiết kế Hệ thống Module cho Phase 5 không?
