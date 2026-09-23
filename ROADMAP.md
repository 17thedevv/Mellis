# Mellis v1.0 — Official Roadmap: The Rust Bootstrap Milestone

Mục tiêu tối thượng của Mellis v1.0 là xây dựng một trình biên dịch hoàn thiện, ổn định và an toàn bằng **Rust**, thay thế hoàn toàn phiên bản C++ ban đầu.
Định nghĩa hoàn thành (Definition of Done) cho v1.0:
1. **Stage 0**: Trình biên dịch C++ (Oracle/Reference hiện tại).
2. **Stage 1**: Trình biên dịch Mellis được viết lại hoàn toàn bằng Rust (`mellis-rs`).
3. **Xác thực**: Đạt 100% Parity (ngữ nghĩa/behavior/output artifact) giữa bản C++ và bản Rust trên toàn bộ codebase thông qua Differential Testing. Trình biên dịch Rust chính thức trở thành primary compiler.

---

## Phase 0 — Compiler Soundness (Nền tảng C++)
*Mục tiêu: Đảm bảo compiler C++ (Stage 0) hoàn toàn vững chắc để đủ sức biên dịch Stage 1.*

- **0.1 Error Propagation (`?`)**:
  - `TryExpr`, tương thích `Option`/`Result`, MVIR lowering.
- **0.2 Lifetime Validation**:
  - Đảm bảo an toàn bộ nhớ tuyệt đối cho `borrow`, `return references`, `closure captures`.
- **0.3 Escape Analysis**:
  - Cấm hoàn toàn việc rò rỉ con trỏ cục bộ (return &local, capture).
- **0.4 Destructuring / Patterns (Hoàn thành)**:
  - Ổn định tuple, struct, match, move, borrow.
- **0.5 Intrinsic Framework**:
  - Rà soát hệ thống, loại bỏ code thừa, chuẩn bị cho runtime.
- **0.6 Generics / Traits / MLib**:
  - Đóng module và liên kết cross-module hoàn thiện.

---

## Phase 1 — Runtime + Core (Môi trường tối thiểu)
*Mục tiêu: Đưa các tính năng cơ bản vào thư viện chuẩn, giúp Mellis chạy mượt mà độc lập.*

- **1.1 Process & Startup**:
  - C FFI: `exit`, `args`, `env`, `current_dir`.
  - Chuỗi khởi động: `entry` → `runtime init` → `main` → `shutdown`.
- **1.2 Hosted Profiles**: Windows và Linux.
- **1.3 Thư viện `core` & `alloc`**:
  - `core`: `Option`, `Result`, `slice`, `str`, `iter`, `cmp`, các Trait chuẩn (`Copy`, `Drop`, v.v.).
  - `alloc`: `Box`, `Vec`, `String` hoàn chỉnh (`String` sử dụng `str` / UTF-8 invariant).

---

## Phase 2 — Self-hosting Prerequisites (Hệ sinh thái OS)
*Mục tiêu: Trình biên dịch Mellis cần thao tác file, path, build process. Các tính năng này trở thành bắt buộc cho v1.0.*

- **2.1 `std::env` (Môi trường)**
  - `var`, `var_or`, `has`, `set_var`, `remove_var`.
  - `vars` (Iterator/Collection toàn bộ biến môi trường).
  - `current_dir`, `set_current_dir`, `args`.
- **2.2 `std::process` (Tiến trình)**
  - Hỗ trợ spawn, run, status, exit để build tools có thể gọi `llvm`, `linker`, `assembler`.
- **2.3 `std::fs` (Hệ thống tệp)**
  - Thao tác: `read_file`, `write_file`, `exists`, `create_dir`, `remove`, `rename`, `metadata`.
- **2.4 `std::path` (Đường dẫn)**
  - Abstraction `Path`, `join`, `parent`, `file_name`, `extension`, `normalize`.
- **2.5 Deterministic MLib Loading**: Tải module mượt mà và chính xác từ file system.

---

🚀 **Mellis v1.0 Public Release** chính thức được phát hành với cấu trúc:
- Trình biên dịch 100% viết bằng Rust, đảm bảo an toàn bộ nhớ và hiệu năng cao.
- Công cụ CLI đi kèm: `mellis build`, `mellis run`, `mellis test`.
- Package chuẩn `std` bao gồm đầy đủ `fs`, `path`, `env`, `process`.
