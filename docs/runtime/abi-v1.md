# Luna Runtime ABI Specification v1.0 (FROZEN)

## 1. Scope & Status
- **Specification Version**: 1.0.0
- **Status**: FROZEN (Milestone RUNTIME-ARCH-01 / Stage 6)
- **Target Profiles**: Hosted (Windows, Linux, macOS), Freestanding (bare-metal, embedded)
- **Calling Convention**: Platform standard C calling convention (`extern "C"`: Microsoft x64 on Windows, System V AMD64 on POSIX).
- **Symbol Namespace**: All runtime public symbols are strictly prefixed with `__luna_*`. All constants and status codes are prefixed with `LUNA_*`.
- **FFI Boundary Principle**: The C runtime deals solely in raw ABI primitives (`const uint8_t*`, `size_t`, `int32_t`, `void*`). High-level language abstractions (`str`, `String`, `Vec`, `Result`, `Option`, rich enums) never cross into native runtime signatures.

---

## 2. Type Mappings Across Compiler, C, and LLVM IR

| Logical Concept | Luna Source | C ABI Type | Rust FFI (`extern "C"`) | LLVM IR Type |
|:---|:---|:---|:---|:---|
| Size / Offset | `usize` | `size_t` | `usize` | `i64` (on 64-bit) |
| Alignment | `usize` | `size_t` | `usize` | `i64` (on 64-bit) |
| Raw Memory Pointer | `*u8` / `*void` | `void*` / `uint8_t*` | `*mut u8` / `*const u8` | `ptr` |
| UTF-8 Byte Sequence | `str` / `&[u8]` | `const uint8_t*, size_t` | `*const u8, usize` | `{ ptr, i64 }` or split `ptr, i64` |
| Exit Code / Status | `i32` | `LunaStatus` (`int32_t`) | `i32` | `i32` |
| Source Location Line | `u32` | `uint32_t` | `u32` | `i32` |
| Source Location Col | `u32` | `uint32_t` | `u32` | `i32` |

---

## 3. Status & Error Model (`LunaStatus`)

The native runtime uses structured status codes defined in `<luna/runtime/abi.h>`. Raw OS integer codes (such as Win32 `GetLastError()` or POSIX `errno`) are strictly normalized inside the runtime and never leak into user code.

All status constants are explicitly defined as 32-bit signed integers:

```c
typedef int32_t LunaStatus;

#define LUNA_STATUS_OK                 ((LunaStatus)0)
#define LUNA_STATUS_EOF                ((LunaStatus)1)
#define LUNA_STATUS_NOT_FOUND          ((LunaStatus)-1)
#define LUNA_STATUS_PERMISSION_DENIED  ((LunaStatus)-2)
#define LUNA_STATUS_ALREADY_EXISTS     ((LunaStatus)-3)
#define LUNA_STATUS_INVALID_ARGUMENT   ((LunaStatus)-4)
#define LUNA_STATUS_IO_ERROR           ((LunaStatus)-5)
#define LUNA_STATUS_OUT_OF_MEMORY      ((LunaStatus)-6)
```

The standard library maps these status codes to high-level semantic enums (e.g. `IoError::{NotFound, PermissionDenied, InvalidInput, InvalidData, Other}`).

---

## 4. Unified Buffer Ownership Model

The Luna Runtime ABI formalizes all memory transfers across the FFI boundary into two canonical categories:

### 4.1 BorrowedBuffer
- **Representation**: `(const uint8_t* ptr, size_t len)`
- **Ownership**: Retained by the runtime / operating system.
- **Contract & Lifetime**:
  Buffers returned by `__luna_process_arg` and `__luna_process_executable` remain valid for the process invocation lifetime (or until the documented runtime shutdown boundary) and must never be deallocated by the caller.
- **Used by**:
  - `__luna_process_arg(index, &ptr, &len)`
  - `__luna_process_executable(&ptr, &len)`

### 4.2 OwnedBuffer
- **Representation**: `(uint8_t* ptr, size_t len, size_t cap)`
- **Ownership**: Transferred completely to caller upon successful return (`LUNA_STATUS_OK`).
- **Invariants**:
  1. $0 \le len \le cap$.
  2. The buffer was allocated from the Luna language heap using `__luna_alloc(cap, 1)`.
  3. Caller releases buffer via `__luna_buffer_free(ptr, cap)` which executes `__luna_dealloc(ptr, cap, 1)`, guaranteeing 100% allocator symmetry even if $len < cap$ (e.g. when a file shrank concurrently during reading).
- **Used by**:
  - `__luna_read_file(path_ptr, path_len, &ptr, &len, &cap)`
  - `__luna_read_line(&ptr, &len, &cap)`

---

## 5. Subsystem Specifications

### 5.1 Memory Allocation Subsystem

#### Alignment Limit
```c
#define LUNA_MAX_ALIGN 4096
```
Every `align` parameter must satisfy:
$$0 < \text{align} \le \text{LUNA\_MAX\_ALIGN} \quad \land \quad (\text{align} \mathrel{\&} (\text{align} - 1)) == 0$$
Any alignment violation triggers an immediate deterministic abort (`LUNA_ERR_INVALID_STATE`).

#### Identity-Based Zero-Size Sentinel
Zero-size allocations return a shared, runtime-owned sentinel object aligned to `LUNA_MAX_ALIGN`:
```c
int __luna_is_zero_sentinel(const void* ptr);
```
Deallocating the sentinel is a guaranteed no-op.

#### Public Memory Functions
- `void* __luna_alloc(size_t size, size_t align);`
  Allocates `size` bytes with alignment `align`. Never returns `NULL` on success; panics deterministically on OOM or invalid alignment.
- `void __luna_dealloc(void* ptr, size_t size, size_t align);`
  Deallocates memory previously allocated by `__luna_alloc` or `__luna_realloc`. Passes exact `size` and `align` corresponding to original allocation. No-op for `NULL` or sentinel.
- `void* __luna_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size);`
  Reallocates block preserving alignment. If `new_size == 0`, frees block and returns sentinel.

---

### 5.2 Panic, Trap & Diagnostics Subsystem

- `LUNA_NORETURN void __luna_panic(const uint8_t* msg_ptr, size_t msg_len, const uint8_t* file_ptr, size_t file_len, uint32_t line, uint32_t col);`
  Formats panic diagnostic to `stderr` and aborts process.
- `LUNA_NORETURN void __luna_panic_default(void);`
  Zero-argument explicit panic entrypoint.
- `LUNA_NORETURN void __luna_bounds_fail(size_t index, size_t len, const uint8_t* file_ptr, size_t file_len, uint32_t line, uint32_t col);`
  Array/slice index bounds failure trap.

---

### 5.3 Process Invocation & Startup Subsystem

Process arguments are strictly separated into user arguments (`argv[1..]`) and executable identification (`argv[0]`):

- `void __luna_startup(int argc, char** argv);`
  Initializes runtime and records process arguments.
- `LUNA_NORETURN void __luna_shutdown(int exit_code);`
  Flushes all standard streams and terminates process via `exit(exit_code)`.
- `LUNA_NORETURN void __luna_abort(void);`
  Immediate abort via `abort()`.
- `size_t __luna_process_arg_count(void);`
  Returns the count of user arguments only (excluding `argv[0]`). If `argc <= 1`, returns 0.
- `int32_t __luna_process_arg(size_t index, const uint8_t** out_ptr, size_t* out_len);`
  Returns borrowed raw byte slice (`BorrowedBuffer`) for user argument at 0-based `index`. Returns `LUNA_STATUS_OK` or `LUNA_STATUS_INVALID_ARGUMENT`.
- `int32_t __luna_process_executable(const uint8_t** out_ptr, size_t* out_len);`
  Returns borrowed raw byte slice (`BorrowedBuffer`) for `argv[0]`. Returns `LUNA_STATUS_OK` or `LUNA_STATUS_NOT_FOUND`.

> **Encoding Contract**:
> The Runtime process-argument ABI exposes platform startup bytes as-is. It does not itself guarantee UTF-8 unless the platform startup implementation explicitly normalizes them. The Luna standard library validates UTF-8 decoding in `process::args()` and safely returns `Result<Vec<String>, ProcessError>` (`ProcessError::InvalidEncoding`) if malformed encodings occur.

---

### 5.4 I/O & File Subsystem

#### Path Handling & Security Invariant
All path parameters are passed as raw byte slices `(const uint8_t* path_ptr, size_t path_len)`.
- **Embedded NUL Defense**: If `memchr(path_ptr, 0, path_len) != NULL`, the runtime immediately rejects the path with `LUNA_STATUS_INVALID_ARGUMENT` preventing truncation security vulnerabilities.
- **Cross-Platform Conversion & Fast-Path Fallback**:
  - Windows: UTF-8 paths are validated and converted to UTF-16 `wchar_t` before invoking Win32 Unicode APIs (`CreateFileW`).
  - Stack buffers (512 wchar on Windows, 1024 bytes on POSIX) serve as allocation-free fast paths.
  - Paths exceeding stack buffer sizes dynamically fall back to checked dynamic heap allocation (`malloc`/`free`), ensuring performance optimizations never impose arbitrary semantic path length restrictions.

#### Functions
- `void __luna_print(const uint8_t* str, size_t len);`
- `void __luna_println(const uint8_t* str, size_t len);`
- `void __luna_eprintln(const uint8_t* str, size_t len);`
- `int32_t __luna_read_file(const uint8_t* path_ptr, size_t path_len, uint8_t** out_ptr, size_t* out_len, size_t* out_cap);`
  Reads entire file into a newly allocated `OwnedBuffer`.
  64-bit size queries (`GetFileSizeEx` on Win32, `fstat` on POSIX).
  Returns `LUNA_STATUS_OK` and sets `*out_ptr`, `*out_len`, and `*out_cap` ($out\_len \le out\_cap$).
  Caller owns buffer and must free with `__luna_buffer_free(*out_ptr, *out_cap)`.
- `int32_t __luna_write_file(const uint8_t* path_ptr, size_t path_len, const uint8_t* data_ptr, size_t data_len);`
  Writes `data_len` bytes to file at path. Overwrites or creates file. Returns `LUNA_STATUS_OK` or negative `LunaStatus`.
- `int32_t __luna_read_line(uint8_t** out_ptr, size_t* out_len, size_t* out_cap);`
  Reads a line from `stdin` up to `\n` or EOF into an `OwnedBuffer`.
  Strips trailing `\n` and preceding `\r` (CRLF normalization).
  Returns:
  - `LUNA_STATUS_OK`: Line read. `*out_len` is logical string length, `*out_cap` is exact allocated buffer capacity.
  - `LUNA_STATUS_EOF`: EOF reached before any bytes read. `*out_ptr = NULL`, `*out_len = 0`, `*out_cap = 0`.
  - Negative `LunaStatus`: I/O or memory error.
- `void __luna_buffer_free(uint8_t* ptr, size_t cap);`
  Releases runtime-allocated buffer with known capacity via `__luna_dealloc(ptr, cap, 1)`.

---

## 6. Architecture & Platform Verification Status

- **Architecture**: COMPLETE & FROZEN ✅
- **ABI v1**: COMPLETE & FROZEN ✅
- **Windows Backend**: VERIFIED ✅ (19/19 native conformance tests passing in `abi_tests.exe`)
- **Linux Backend**: IMPLEMENTED — PLATFORM CONFORMANCE PENDING (POSIX pthreads sync and threading implementations written and integrated into CMake; native conformance suite execution pending Linux CI environment)
