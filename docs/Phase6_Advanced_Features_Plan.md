# Phase 8: Ecosystem, Standard Library & Advanced Features Plan

Tài liệu này vạch ra kiến trúc và lộ trình triển khai chi tiết cho các tính năng nâng cao còn thiếu ở Backend (LLVM IR Generation) và Middle-End của FD-Lang. Các tính năng này đã được Parser hỗ trợ nhưng cần chuyển đổi thành mã máy LLVM.

## 1. Lambda Expressions (Closures)

### Khái quát
Lambda (Hàm nặc danh) trong FD-Lang không chỉ là con trỏ hàm (function pointer) mà còn phải hỗ trợ **Bắt biến môi trường (Capturing)**. Điều này biến nó thành một **Closure**.

### Kiến trúc & Các bước triển khai
- **Middle-End (TypeChecker)**:
  - Khi gặp `LambdaExpr`, trình biên dịch cần quét thân hàm để tìm các biến thuộc phạm vi (scope) bên ngoài.
  - Tự động sinh ra một **Struct ẩn (Anonymous Struct)** để lưu trữ các biến bị bắt (captured variables).
  - Khởi tạo một kiểu dữ liệu mới nội bộ (ví dụ: `Closure_123`) cài đặt một trait ẩn Callable.
- **Back-End (MVIR & LLVM)**:
  - Sinh mã khởi tạo struct ẩn trên Heap (nếu thoát khỏi scope) hoặc Stack.
  - Hạ cấp (Lowering) lời gọi lambda thành việc truyền struct này vào như tham số ngầm (giống `self` của phương thức).

---

## 2. Async / Await (Coroutines)

### Khái quát
Cơ chế lập trình bất đồng bộ. Trong FD-Lang, `async fn` không trả về giá trị trực tiếp mà trả về một cấu trúc dạng `Future<T>`.

### Trạng thái hiện tại (MVP)
- **TypeChecker**: Đã hỗ trợ kiểu `Future<T>` nội tại (`{ ptr, i8, T }`) và ép kiểu trả về của hàm `async`.
- **LLVM IR**: Đã tích hợp thành công tập lệnh LLVM Coroutines (`llvm.coro.id`, `llvm.coro.size`, `llvm.coro.begin`, `llvm.coro.end`).
- **Hiện trạng**: Coroutine hiện tại chạy đồng bộ (synchronous) từ đầu đến cuối do chưa cài đặt điểm dừng (Suspend point). Lệnh `await` hiện chỉ lấy giá trị ra từ cấu trúc `Future<T>`.

### Kiến trúc & Các bước triển khai tiếp theo
- **Suspend Points (`llvm.coro.suspend`)**:
  - Tại lệnh `await`, compiler phải sinh mã gọi `@llvm.coro.suspend` để nhường quyền điều khiển (yield) lại cho hàm gọi (caller).
  - Bổ sung state logic cho `AwaitInst` trong MVIR để theo dõi tính hoàn thành của `Future`.
- **C-Runtime Executor (Standard Library)**:
  - Xây dựng hệ thống runtime trong file `lib/std` (viết bằng chính `fdlang` kết hợp C-FFI) chứa hàm `block_on` và một Executor đơn giản (Event Loop).
  - `block_on` sẽ nhận vào Coroutine Handle, gọi `@llvm.coro.resume` lặp đi lặp lại cho đến khi cờ trạng thái báo hoàn thành (Done), sau đó trích xuất kết quả `T` trả về cho chương trình chính (`main`).

---

## 3. Comptime (Compile-Time Execution)

### Khái quát
Cho phép thực thi trực tiếp các khối lệnh hoặc hàm `comptime` ngay trong quá trình biên dịch (tương tự `constexpr` của C++ hoặc `comptime` của Zig).

### Kiến trúc & Các bước triển khai
- **Middle-End**:
  - Đánh dấu vùng không gian biến dành riêng cho Comptime.
  - Đảm bảo các hàm/biến được gọi trong `comptime` không có tác dụng phụ ngoại cảnh (Pure functions, không đọc file/mạng nếu không được phép).
- **Thực thi (Execution)**:
  - Xây dựng một **AST Interpreter** nhỏ, hoặc sử dụng **LLVM ORC JIT** để biên dịch nhanh đoạn code đó thành mã máy, chạy ngay trong bộ nhớ của trình biên dịch (mellis), lấy kết quả và nhúng (embed) vào LLVM IR chính dưới dạng Hằng số (Constant).

---

## 4. Unsafe Blocks

### Khái quát
Cung cấp "cửa hậu" (escape hatch) cho phép lập trình viên vượt rào Borrow Checker để thao tác trực tiếp với con trỏ thô (raw pointers), gọi C-FFI, và ép kiểu nguy hiểm.

### Hiện trạng (v1.0)
- **Đã hoạt động một phần**: Khối `unsafe {}` đã được Lexer và Parser ghi nhận. Trình biên dịch hiện tại đã cho phép lấy con trỏ thô (Raw Pointers) và thao tác với chúng thông qua ép kiểu (cast). Tuy nhiên, cần thắt chặt hơn ranh giới giữa an toàn và không an toàn.

### Kiến trúc & Các bước triển khai
- **Middle-End (TypeChecker / Borrow Checker)**:
  - Khi phân tích vào một `UnsafeStmtNode`, bật cờ `is_unsafe_context = true`.
  - Các thao tác bị cấm ở ngữ cảnh bình thường (như dereference `*ptr` của con trỏ thô, hoặc gọi hàm `extern fn` không an toàn) sẽ được cho phép nếu cờ này đang bật.
  - Đảm bảo tính đóng gói: Ranh giới của `unsafe` không làm hỏng tính an toàn của các biến bên ngoài khối lệnh.
- **Back-End**:
  - Không thay đổi mã LLVM IR được sinh ra. Backend chỉ dịch mọi thứ như bình thường; rào cản chỉ tồn tại ở Middle-End.

---

## Lộ trình ưu tiên cho Phase 8 (Ecosystem)
1. **Standard Library (lib/std) & Core Utilities**: Bắt đầu xây dựng thư viện chuẩn viết bằng fdlang, bao gồm các cấu trúc dữ liệu nền tảng như `String`, `Vec`, `File`.
2. **Hoàn thiện Async/Await (Executor & Suspend)**: Đi kèm với Standard Library, cần hoàn thiện khâu tạm dừng luồng (`suspend`) và viết Runtime Executor (`block_on`) bằng chính fdlang.
3. **Lambda / Closures**: Đòi hỏi thay đổi TypeChecker để tạo Struct ẩn và cập nhật LLVM IR (Đã có kế hoạch kiến trúc chi tiết).
4. **Comptime**: Phức tạp nhất và độc lập nhất, yêu cầu tích hợp LLVM ORC JIT hoặc viết thêm AST Interpreter.
