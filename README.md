# Mellis Compiler (FDLang) - Version 1.0

Mellis là một trình biên dịch (compiler) dành cho ngôn ngữ **FDLang**, được thiết kế hướng tới ngôn ngữ lập trình hệ thống (System Programming Language) hiện đại, hiệu năng cao, và an toàn. Trình biên dịch sử dụng kiến trúc phân tầng chuyên nghiệp, tạo ra mã trung gian (MVIR) riêng biệt và biên dịch ra mã máy (Native Machine Code) thông qua backend **LLVM**.

## ✨ Tính năng nổi bật

- **Kiến trúc Hiện đại (Modern Architecture)**:
  - Phân tách rõ ràng các tầng Front-end (Lexer, Parser), Middle-end (Type Checker, Borrow Checker, MVIR), và Back-end (LLVM IR Generator).
  - Tích hợp sẵn cơ chế kiểm tra toàn vẹn bộ nhớ cơ bản (Borrow Checking) và Liveness Analysis.
- **Pattern Matching mạnh mẽ**: Hỗ trợ bóc tách cấu trúc dữ liệu, Enum Variant với các ràng buộc kiểu tĩnh chặt chẽ.
- **Tail Expressions**: Hỗ trợ biểu thức khối lệnh ngầm định trả về (Implicit return) mà không cần từ khóa `return` hay dấu chấm phẩy `;`.
- **Hệ thống Module (MLib)**: Hỗ trợ biên dịch nhị phân thành file `.mlib` (Mellis Library) giúp nạp và tăng tốc biên dịch các thư viện chuẩn (Standard Library) mà không cần biên dịch lại từ mã nguồn.
- **Diagnostics thông minh (LSP Ready)**: Báo lỗi có màu sắc (ANSI ANSI colors), đánh dấu caret (`^`) chính xác vị trí lỗi, dễ dàng mở rộng sang chuẩn **Language Server Protocol (LSP)** cho các IDE như VSCode.
- **Portable Toolchain**: Khả năng phân phối trình biên dịch và thư viện chuẩn theo cấu trúc `bin`/`lib` mà không bị phụ thuộc vào biến môi trường hệ thống. Tự động tương thích với bộ liên kết (linker) như LLD.

## 🚀 Cấu trúc dự án

- `src/FrontEnd/`: Chứa Lexer, Parser để xây dựng Cây Cú Pháp Trừu Tượng (AST).
- `src/MiddleEnd/`: Phân tích ngữ nghĩa (Semantic), kiểm tra kiểu (Type Checker), Borrow Checker, và mã trung gian MVIR.
- `src/BackEnd/`: Dịch MVIR sang LLVM IR và gọi Linker để tạo file thực thi (`.exe`).
- `src/MLib/`: Hệ thống nạp (Loader) và sinh (Generator) metadata thư viện.
- `src/Support/`: Các tiện ích dùng chung (Diagnostic Engine, OSUtils, v.v.).

## 📖 Tài liệu Tham khảo (Documentation)

- [Language Reference](file:///d:/fdlang/docs/LanguageReference.md): Khái quát tất cả tính năng của ngôn ngữ Mellis (Primitive types, Control Flow, Structs, Enums & Pattern Matching, Traits & Generics, Borrowing, Closures, Macros...).
- [Architecture](file:///d:/fdlang/docs/architecture.md): Cấu trúc kiến trúc nội bộ của Mellis Compiler.
- [MVIR](file:///d:/fdlang/docs/MVIR.md): Đặc tả mã trung gian Mellis Virtual Intermediate Representation.

## 🛠️ Hướng dẫn Build

Mellis yêu cầu trình biên dịch C++20 và bộ thư viện **LLVM 18+** được cấu hình trên máy tính.

1. Clone mã nguồn:
   ```bash
   git clone <repo-url> fdlang
   cd fdlang
   ```

2. Tạo thư mục build và cấu hình CMake:
   ```bash
   cmake -B build -S .
   ```

3. Tiến hành Build dự án:
   ```bash
   cmake --build build --config Release
   ```

## 📦 Cách sử dụng (Portable Toolchain)

Mellis được thiết kế để dễ dàng phân phối. Bạn có thể xây dựng cấu trúc công cụ hoàn chỉnh như sau:

```text
fdlang/
├── bin/
│   ├── mellis.exe         <-- Trình biên dịch Mellis
│   └── lld-link.exe       <-- Linker của LLVM (tùy chọn để portable)
└── lib/
    ├── std.mlib           <-- Thư viện chuẩn (Standard Library)
    └── ...
```

Để biên dịch một file mã nguồn FDLang (`.ms`), sử dụng lệnh:

```bash
mellis main.ms
```

Để xem toàn bộ quá trình phân tích (Debug Logs):

```bash
mellis main.ms --verbose
```

Xem phiên bản Mellis:
```bash
mellis --version
```

## 📜 Giấy phép
Dự án được phát triển nội bộ cho FDLang.
