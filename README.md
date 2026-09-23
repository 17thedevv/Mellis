# Luna Compiler & Toolchain

**Luna** (trước đây có tên lịch sử là *Mellis* / *fdlang*) là ngôn ngữ lập trình hệ thống hiện đại, hướng tới hiệu năng cao, an toàn bộ nhớ tĩnh và khả năng kiểm soát chặt chẽ tài nguyên. Trình biên dịch canonical của Luna được hiện thực hoàn toàn bằng **Rust** (nằm tại `luna-rs/`), sinh mã trung gian MVIR và dịch sang mã máy native thông qua **LLVM**.

## ✨ Kiến trúc cốt lõi

- **Trình biên dịch Canonical (`luna`)**: Được tổ chức thành workspace modular bằng Rust tại `luna-rs/crates/*` (`luna-lexer`, `luna-parser`, `luna-semantic`, `luna-mvir`, `luna-backend`, `luna-borrowck`, `luna-llib`, `luna-driver`, `luna-cli`).
- **Thư viện chuẩn Component-level (STD-ARCH-01 FROZEN)**: Hệ thống 25 provider module hóa độc lập (`alloc`, `core`, `io`, `lang`) nằm tại `luna-rs/libs/external/`. Mỗi provider sở hữu bộ ba hoàn chỉnh: `.ln` (mã nguồn), `.llib` (metadata & MVIR), `.obj` (mã máy native).
- **Nguyên lý Định danh Artifact ("Luna trusts identity, never existence")**: Hệ thống nhận diện artifact dựa trên typed fingerprint (source identity, interface identity, target contract) đảm bảo không có rò rỉ hay stale artifact fallback.
- **Hệ thống Kiểm tra An toàn Bộ nhớ**: Tích hợp Borrow Checker, Liveness Analysis, Escape Analysis và quy tắc Lifetime rõ ràng (`where outlives`, `life_from`).
- **Runtime Canonical (`__luna_*`, RUNTIME-RENAME-01 FROZEN)**: Native C runtime nằm tại `runtime/` (`luna-runtime.lib`) cung cấp các hàm nền tảng (`__luna_alloc`, `__luna_dealloc`, `__luna_print`, `__luna_println`, `__luna_panic`, `__luna_startup`, `__luna_shutdown`) theo chuẩn ABI prefix `__luna_*` và headers `luna/runtime/*`.

## 📁 Cấu trúc Dự án

- `luna-rs/`: Toàn bộ mã nguồn Rust của trình biên dịch Luna và sysroot canonical.
  - `crates/`: 12 crate thành viên của workspace Luna compiler.
  - `libs/external/`: Thư viện chuẩn canonical 25 component-level providers (`sysroot.toml`).
- `runtime/`: Thư viện native C runtime (`luna-runtime.lib`) và các bài kiểm tra ABI conformance.
- `docs/`: Tài liệu kiến trúc, ngôn ngữ tham chiếu và các báo cáo freeze (STD-ARCH-01, RUNTIME-RENAME-01).
- `tests/`: Bộ test fixtures ngôn ngữ của Luna.

## 🛠️ Hướng dẫn Build & Test

Yêu cầu: Rust (Cargo 1.80+), LLVM và MinGW GCC/Clang hoặc MSVC (cho native runtime).

### 1. Build Trình biên dịch Luna (Rust)
```bash
cd luna-rs
cargo build -p luna-cli
```

### 2. Chạy Kiểm thử Toàn bộ Workspace
```bash
cd luna-rs
cargo test --workspace
```

## 📜 Ghi chú Lịch sử
- **Mellis** và **fdlang** là tên lịch sử của dự án trước khi đổi tên sang **Luna**.
- Mọi mã nguồn production, runtime ABI (`__luna_*`), headers (`luna/runtime/*`), thư viện (`luna-runtime.lib`), artifact (`.llib`) và biến môi trường (`LUNA_*`) tuân thủ nghiêm ngặt định danh Luna duy nhất theo RUNTIME-RENAME-01.
