# Phase 6 — Core Completion & QoL

## Mục tiêu
Hoàn thiện toàn bộ feature core còn thiếu trong Status.md, đồng thời kiểm tra end-to-end:
Parser → AST → Resolver → TypeChecker / TraitSolver → BorrowChecker → MVIR → LLVM → MLib → Diagnostics

**Quy tắc 100%:**
Một feature chỉ được đánh dấu ✅ 100% khi có: Syntax, Semantic, Lowering, Backend, Diagnostics, Positive tests, Negative tests, Cross-feature tests.

## Lộ trình triển khai

1. **Target 6.1 — Destructuring**: Hợp nhất destructuring với Pattern infrastructure hiện có (Tuple, Struct, Enum, For-in).
2. **Target 6.2 — Type Alias**: Alias transparent (ví dụ `type UserId = uint_64;`), Generic alias.
3. **Target 6.3 — Const**: Compile-time constant semantics (`const MAX = 1024;`), evaluate compile-time.
4. **Target 6.4 — Static / Global Data**: `static counter: int_32 = 0;`, quản lý linkage, visibility, thread-safety, LLVM globals.
5. **Target 6.5 — Array & Slice Completion**: Hoàn thiện `Array<T, N>` và `Slice<T>`, Bounds checking (compile-time OOB error / dynamic OOB trap), Memory safety.
6. **Target 6.6 — Control Flow Hardening**: Nested `break`/`continue`/`return` trong `while`, `for`, `match`. Đảm bảo cleanup/drop chính xác trên mọi exit path.
7. **Target 6.7 — Multiple Trait Bounds**: Hỗ trợ `fn foo<T: Clone + Display>(x: T)`.
8. **Target 6.8 — Operator Overloading**: TraitSolver dựa vào Core registry (`Add`, `Sub`, `Eq`...). Primitive fast path & custom Trait fallback.
9. **Target 6.9 — Index Overloading**: `Index` và `IndexMut` cho `arr[i]`.
10. **Target 6.10 — Pointer Arithmetic**: `ptr + n`, lower xuống GEP, check unsafe boundary.
11. **Target 6.11 — Callable / Function Types**: Closure, function pointer, generic function passed as value.
12. **Target 6.12 — Type / Generic Edge Audit**: Nested generics, generic enum/struct/trait kết hợp. Đóng các lỗ hổng edge cases.
13. **Target 6.13 — Pattern & Match Final Audit**: Nested enum, destructuring, exhaustive checker (unreachable pattern warning/error).
14. **Target 6.14 — Diagnostics Completion**: Semantic error → one primary diagnostic + related candidate notes (không cascade vô nghĩa).
15. **Target 6.15 — Cross-Layer Final Audit**: Freeze criteria trước v1.0.

## Freeze Criteria mới
Mellis v1.0 chỉ chuyển sang "Language Core v1.0 Complete" khi:
- Tất cả core features = 100%
- Không còn TODO semantic trong core
- Không có feature chỉ “parse được”
- Các bài Test pass (positive, negative, cross-feature)
- IRVerifier, MLib round-trip, Deterministic build pass
- Không thêm syntax mới trừ khi là điều kiện bắt buộc hoàn thành core scope.
