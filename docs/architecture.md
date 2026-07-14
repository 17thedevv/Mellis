///
1. Tổng quan (Overview & Philosophy)
Tên ngôn ngữ: freedomLanguage (fdlang)

Định dạng file: .fl (Source file) và .flh (Header/Interface file tùy chọn). Trình biên dịch đối xử với cả hai đuôi này như nhau, trao toàn quyền tổ chức code cho người dùng.

Mục tiêu thiết kế: Cú pháp hiện đại theo phong cách riêng, an toàn, biên dịch ra mã máy native qua LLVM. đủ mạnh để làm game engine mượt mà.

///
2. Kiến trúc Trình biên dịch (Compiler Pipeline)
Mô tả luồng đi của dữ liệu qua các module (C++):

Frontend: Lexer -> Parser (Recursive Descent) -> Semantic Analyzer (Type & Scope Checking).

Backend: LLVM IR Generator -> LLVM Optimizer Passes -> Machine Code Emission (AOT/JIT).

///
3. Hệ thống Module (Module System & Visibility)
Cơ chế Import: Sử dụng Semantic Import (Giao tiếp qua Bảng ký hiệu), không dùng Textual Inclusion (#include kiểu C).

Đóng/Mở (Encapsulation): Mặc định mọi biến/hàm là Private. Bắt buộc dùng từ khóa export để phơi bày ra cho file khác import.

///
4. Hệ thống Kiểu & Quản lý bộ nhớ (Type System & Memory)
MVP Memory: Mọi biến nguyên thủy (int, float, bool) sống trên Stack, do LLVM tự động dọn dẹp bằng alloca. Chưa tích hợp Heap/GC trong Phase 1.

Biến và Hằng số:

let: Biến có thể thay đổi (Mutable).

const: Hằng số bắt buộc gán ngay, trình biên dịch báo lỗi nếu gán lại.

///
5. Đặc tả Ngữ pháp (Grammar Specification)

///
Thay vì để các module tự in lỗi ra màn hình một cách vô tổ chức, toàn bộ kiến trúc trên được cắm vào một module dùng chung gọi là Diagnostic Engine.

Khi Lexer gặp ký tự lạ ➔ Bắn tín hiệu cho Diagnostic Engine.

Khi Parser thấy thiếu dấu ; ➔ Bắn tín hiệu.

Khi Type Checker thấy cộng int với string ➔ Bắn tín hiệu.

Engine này sẽ thu thập tất cả, format lỗi thật đẹp (có chỉ mũi tên vào đúng dòng/cột bị sai như Rust) rồi mới in ra cho người dùng.