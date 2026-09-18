# Luna Compiler & Toolchain

**Luna** (trước đây có định danh lịch sử là *Mellis* / *fdlang*) là ngôn ngữ lập trình hệ thống hiện đại, hướng tới hiệu năng cao, an toàn bộ nhớ tĩnh và khả năng kiểm soát chặt chẽ tài nguyên. Trình biên dịch canonical của Luna được hiện thực hoàn toàn bằng **Rust** (nằm tại `mellis-rs/`), sinh mã trung gian MVIR và dịch sang mã máy native thông qua **LLVM**.

## ✨ Kiến trúc cốt lõi

- **Trình biên dịch Canonical (`luna`)**: Được tổ chức thành workspace modular bằng Rust tại `mellis-rs/crates/*` (`luna-lexer`, `luna-parser`, `luna-semantic`, `luna-mvir`, `luna-backend`, `luna-borrowck`, `luna-llib`, `luna-driver`, `luna-cli`).
- **Thư viện chuẩn Component-level (STD-ARCH-01 FROZEN)**: Thay thế kiến trúc monolithic cũ bằng 25 provider module hóa độc lập (`alloc`, `core`, `io`, `lang`) nằm tại `mellis-rs/libs/external/`. Mỗi provider sở hữu bộ ba hoàn chỉnh: `.ln` (mã nguồn), `.llib` (metadata & MVIR), `.obj` (mã máy native).
- **Nguyên lý Định danh Artifact ("Luna trusts identity, never existence")**: Hệ thống nhận diện artifact dựa trên typed fingerprint (source identity, interface identity, target contract) đảm bảo không có rò rỉ hay stale artifact fallback.
- **Hệ thống Kiểm tra An toàn Bộ nhớ**: Tích hợp Borrow Checker, Liveness Analysis, Escape Analysis và quy tắc Lifetime rõ ràng (`where outlives`, `life_from`).
- **Runtime ABI Frozen (`__mellis_*`)**: Runtime tối thiểu viết bằng C tại `runtime/` cung cấp các hàm nền tảng (`__mellis_alloc`, `__mellis_dealloc`, `__mellis_print`, `__mellis_println`, `__mellis_panic`) được bảo tồn có chủ đích theo hợp đồng ABI đã đóng băng.

## 📁 Cấu trúc Dự án

- `mellis-rs/`: Toàn bộ mã nguồn Rust của trình biên dịch Luna và sysroot canonical.
  - `crates/`: 12 crate thành viên của workspace Luna compiler.
  - `libs/external/`: Thư viện chuẩn canonical 25 component-level providers (`sysroot.toml`).
- `runtime/`: Thư viện native C runtime (`mellis-runtime.lib`) và các bài kiểm tra ABI conformance.
- `docs/`: Tài liệu kiến trúc, ngôn ngữ tham chiếu và các báo cáo freeze (STD-ARCH-01).
- `tests/`: Bộ test fixtures ngôn ngữ của Luna.

## 🛠️ Hướng dẫn Build & Test

Yêu cầu: Rust (Cargo 1.80+), LLVM và CMake (cho native runtime).

### 1. Build Trình biên dịch Luna (Rust)
```bash
cd mellis-rs
cargo build -p luna-cli
```

### 2. Chạy Kiểm thử Toàn bộ Workspace
```bash
cd mellis-rs
cargo test --workspace
```

## 📜 Ghi chú Lịch sử & Hợp đồng Tương thích
- Tên gọi **Mellis** và **fdlang** là tên lịch sử của dự án trước khi chuyển sang **Luna**.
- Các định danh ABI runtime dạng `__mellis_*` được cố ý giữ nguyên nhằm duy trì tính ổn định của ABI.
- Khả năng đọc mã nguồn `.ms` và thư viện `.mlib` cũ được duy trì như một kênh tương thích ngược theo ma trận `COMPAT-*`.
