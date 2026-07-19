# 📋 Trạng thái Dự án — freedomLanguage (mellis)

| Hạng mục | Giá trị |
|----------|---------|
| **Version** | `v1.0` |
| **Branch** | `main` |
| **Ngôn ngữ triển khai** | C++17 |
| **Build system** | CMake 3.20+ |
| **Backend** | LLVM |
| **Cập nhật lần cuối** | 2026-07-19 |

> ⚠️ **v1.0 — Đây là bản compiler sẽ dùng trong thực tế. Làm cẩn thận, không đốt cháy giai đoạn.**

---

## ✅ Các cấu trúc đã biên dịch được (Verified)

### 🔢 Kiểu dữ liệu nguyên thủy
| Cú pháp | Trạng thái |
|---------|-----------|
| `int_8`, `int_16`, `int_32`, `int_64` | ✅ |
| `uint_8`, `uint_16`, `uint_32`, `uint_64` | ✅ |
| `float_32`, `float_64` | ✅ |
| `bool` | ✅ |
| `str` (C string literal) | ✅ |
| `void` | ✅ |

### 🏗️ Khai báo & Hàm
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| Khai báo biến | `dec x: int_32 = 5;` | ✅ |
| Khai báo + type inference | `dec x = 42;` | ✅ |
| Hàm | `fn foo(a: int_32) -> int_32 { ... }` | ✅ |
| Export hàm | `export fn main() -> int_32 { ... }` | ✅ |
| FFI / extern | `extern fn printf(fmt: str, ...) -> int_32;` | ✅ |
| Varargs (`...`) | `extern fn printf(fmt: str, ...) -> int_32;` | ✅ |
| Generic hàm | `fn add@<T>(a: T, b: T) -> T { ... }` | ✅ |

### 🔀 Toán tử & Biểu thức
| Cú pháp | Trạng thái |
|---------|-----------|
| Arithmetic: `+`, `-`, `*`, `/`, `%` | ✅ |
| Comparison: `>`, `<`, `>=`, `<=`, `==`, `!=` | ✅ |
| Logic: `&&`, `\|\|`, `!` | ✅ |
| Bitwise: `&`, `\|`, `^`, `<<`, `>>` | ✅ |
| Gọi hàm: `foo(a, b)` | ✅ |
| Type cast: `x as int_8` | ✅ |
| `sizeof(int_32)` | ✅ |
| `alignof(int_32)` | ✅ |
| Gán: `x = expr` | ✅ |

### 🔁 Luồng điều khiển
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| `if` | `if x > 0 { ... }` | ✅ |
| `if/else` | `if x > 0 { ... } else { ... }` | ✅ |
| `while` | `while i < 10 { i = i + 1; }` | ✅ |
| `for` (C-style) | `for (dec i = 0; i < 5; i = i+1) { }` | ✅ |
| `return` | `return x;` | ✅ |
| `match` (literal + wildcard) | `match x { 1 -> 10, _ -> 0 }` | ✅ |

### 🧱 Struct & Field
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| Struct định nghĩa | `struct Point { x: int_32; y: int_32; }` | ✅ |
| Struct khởi tạo | `Point { x: 10, y: 20 }` | ✅ |
| Field access | `p.x`, `p.y` | ✅ |
| Generic struct | `struct Box<T> { value: T; }` | ✅ |
| Generic struct init | `Box@<int_32>{ value: 42 }` | ✅ |
| Impl block (methods) | `impl Box<T> { fn get(self: ...) -> T { ... } }` | ✅ |
| Method call | `b.get()` | ✅ |
| Generic impl monomorphization | `impl<T> Box<T>` → concrete `Box<int_32>` | ✅ |

### 📦 Enum
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| Enum đơn giản | `enum Color { Red, Green, Blue }` | ✅ |
| Enum variant access | `Color::Red` | ✅ |
| Generic enum | `enum Option<T> { Some(T), None }` | ✅ |
| Enum pattern matching | `match opt { Option::Some(x) -> x, ... }` | ✅ |
| Enum với data | `Option::Some(42)` | ✅ |

### 📐 Array & Slice
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| Array literal | `[1, 2, 3, 4, 5]` | ✅ |
| Array indexing | `arr[2]` | ✅ |

### 🔗 Module System
| Cú pháp | Ví dụ | Trạng thái |
|---------|-------|-----------|
| `mod` import | `mod math;` | ✅ |
| `use` alias | `use math as m;` | ✅ |
| Qualified call | `m::add(10, 20)` | ✅ |

---

## ❌ Chưa hoạt động / Còn lỗi

| Tính năng | Vấn đề | Mức độ |
|-----------|--------|--------|
| **Tuple access** `t.0`, `t.1` | Parser lỗi "Expected member name" khi gặp index số | 🔴 Cao |
| **`if/else` với `return` trong else branch** | LLVM IR sinh sai (thiếu terminator) | 🔴 Cao |
| **`for in` iterable** | Chưa implement (`for x in arr`) | 🟡 Trung bình |
| **Generic enum pattern matching** | Phức tạp hơn — cần test kỹ hơn | 🟡 Trung bình |
| **Trait objects `dyn Trait`** | Chưa implement (cần VTable) | 🟡 Trung bình |
| **String type nội tại** | Chỉ có C string literal `str`, chưa có heap string | 🟡 Trung bình |
| **`break` / `continue`** | Chưa test | 🟡 Trung bình |
| **Diagnostic line/col** | Tất cả lỗi chỉ báo byte offset, không có line:col | 🟢 Thấp |

---

## 🏗️ Kiến trúc Pipeline

```
Source (.ms)
    │
    ▼
[FrontEnd]
    Lexer       ✅ hoàn chỉnh
    ↓
    Parser      ✅ hoàn chỉnh (Recursive Descent)
    ↓
    AST         ✅ đầy đủ nodes

[MiddleEnd]
    Resolver           ✅ Two-pass, scoped symbol table
    ↓
    Type Checker       ✅ Constraint-Based, Generics, Traits
    ↓
    Monomorphization   ✅ Generic impl instantiation
    ↓
    Borrow Checker     ✅ Scope-based ownership
    ↓
    MVIR Generator     ✅

[BackEnd]
    LLVM IR Generator  ✅
    ↓
    Executable Gen     ✅ (lld-link)
```

---

## 📊 Tiến độ thực tế

```
Phase 1 — Lexer                 ██████████  100% ✅
Phase 1 — Parser                █████████░   90% (tuple index, for-in)
Phase 1 — AST                   ██████████  100% ✅

Phase 2 — Resolver              ██████████  100% ✅
Phase 2 — Type System           ██████████  100% ✅
Phase 2 — TypeChecker Core      ██████████  100% ✅
Phase 2 — TypeChecker Generics  ████████░░   80% ✅ (monomorphization done)
Phase 2 — TypeChecker Traits    ██████░░░░   60% (no dyn Trait yet)
Phase 2 — Borrow Checker        ██████████  100% ✅
Phase 2 — MVIR Generator        ██████████  100% ✅

Phase 3 — LLVM IR Generator     █████████░   90% (if/else edge case)
Phase 3 — Executable Gen        ██████████  100% ✅

Diagnostic Engine               ███████░░░   70% (line/col chưa có)
────────────────────────────────────────────────────
Tổng thể v1.0                   █████████░  ~88%
```
