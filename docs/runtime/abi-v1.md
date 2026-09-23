# Mellis Runtime ABI Specification v1.0 (FROZEN)

## 1. Scope & Status
- **Specification Version**: 1.0.0
- **Status**: FROZEN
- **Target Profiles**: Hosted (Windows, Linux, macOS), Freestanding (bare-metal, embedded)
- **Calling Convention**: Platform standard C calling convention (`extern "C"`: Microsoft x64 on Windows, System V AMD64 on POSIX).
- **Symbol Namespace**: All runtime public symbols are strictly prefixed with `__mellis_`. No auxiliary symbols or un-namespaced identifiers (`malloc`, `free`, `puts`, `printf`, `main`) are exposed by the Mellis runtime library.

---

## 2. Type Mappings Across Compiler, C, and LLVM IR

| Logical Concept | Mellis Source | C ABI Type | Rust FFI (`extern "C"`) | LLVM IR Type |
|:---|:---|:---|:---|:---|
| Size / Offset | `usize` | `size_t` | `usize` | `i64` (on 64-bit) |
| Alignment | `usize` | `size_t` | `usize` | `i64` (on 64-bit) |
| Raw Memory Pointer | `*u8` / `*void` | `void*` / `uint8_t*` | `*mut u8` / `*const u8` | `ptr` |
| UTF-8 Byte Sequence | `str` / `&[u8]` | `const uint8_t*, size_t` | `*const u8, usize` | `{ ptr, i64 }` or split `ptr, i64` |
| Exit Code / Status | `i32` | `int32_t` | `i32` | `i32` |
| Source Location Line | `u32` | `uint32_t` | `u32` | `i32` |
| Source Location Col | `u32` | `uint32_t` | `u32` | `i32` |

---

## 3. The 9 Core Runtime ABI Functions

### 3.1 Startup & Entrypoint Bridge (Model B: Compiler-Owned Entrypoint)

#### `__mellis_startup`
```c
void __mellis_startup(int argc, char** argv);
```
- **Direction**: **Exported** by runtime, **Invoked** by compiler-generated `@main`.
- **Role**: Initializes runtime subsystems (stores `argc`/`argv`, initializes standard handles).
- **Contract**: Non-terminal; returns cleanly to let execution proceed.

#### `__mellis_start`
```c
int32_t __mellis_start(int argc, char** argv);
```
- **Direction**: **Emitted** by compiler for executable targets, **Invoked** by compiler-generated `@main`.
- **Role**: The compiler-generated executable entry shim separating OS arguments from Mellis semantics.
- **Contract**:
  1. If user `main` accepts `(args: [str])`: constructs slice `[str]` from `argv[0..argc]`.
  2. Invokes user-defined `__mellis_user_main(...)`.
  3. If user `main` returns `void`: normalizes return code to `0`.
  4. If user `main` returns `i32`: propagates the return code directly.
  5. Drops temporary arguments allocation if any.
  6. Returns normalized `i32` exit code to `@main`.

#### `__mellis_shutdown`
```c
MELLIS_NORETURN void __mellis_shutdown(int32_t exit_code);
```
- **Direction**: **Exported** by runtime, **Invoked** by compiler-generated `@main`.
- **Role**: Flushes stdio streams and terminates process via platform exit (`exit(exit_code)`).
- **Contract**: Terminal `[[noreturn]]`. In LLVM IR, must be followed immediately by `unreachable`.

---

### 3.2 Memory Allocation Subsystem

#### Alignment Limit
```c
#define MELLIS_MAX_ALIGN 4096
```
- Every `align` parameter must satisfy:
  $$0 < \text{align} \le \text{MELLIS\_MAX\_ALIGN} \quad \land \quad (\text{align} \mathrel{\&} (\text{align} - 1)) == 0$$
- Any alignment violation triggers an immediate deterministic abort (`MELLIS_ERR_INVALID_STATE`).

#### Identity-Based Zero-Size Sentinel
- Zero-size allocations return a shared, runtime-owned sentinel object aligned to `MELLIS_MAX_ALIGN`:
  ```c
  static _Alignas(MELLIS_MAX_ALIGN) const unsigned char g_mellis_zero_sentinel[1] = { 0 };

  static inline int is_zero_sentinel(const void* ptr) {
      return ptr == (const void*)g_mellis_zero_sentinel;
  }
  ```

#### `__mellis_alloc`
```c
void* __mellis_alloc(size_t size, size_t align);
```
- **Parameters**:
  - `size`: Number of bytes to allocate.
  - `align`: Memory alignment requirement ($0 < \text{align} \le \text{MELLIS\_MAX\_ALIGN}$, power of 2).
- **Return**:
  - If `size == 0`: Returns `(void*)g_mellis_zero_sentinel`.
  - If `size > 0`: Returns non-null aligned pointer satisfying $ptr \pmod{align} == 0$.
- **Failure Contract**:
  - Round-up overflow (`size > SIZE_MAX - (align - 1)`) or OOM aborts immediately via `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE, ...)`.
  - **Never returns `NULL`** in Mellis v1.0.

#### `__mellis_dealloc`
```c
void __mellis_dealloc(void* ptr, size_t size, size_t align);
```
- **Parameters**:
  - `ptr`: Pointer previously returned by `__mellis_alloc` or `__mellis_realloc`.
  - `size`: Allocation size metadata.
  - `align`: Allocation alignment metadata.
- **Contract**:
  - If `ptr == NULL || is_zero_sentinel(ptr)`: Returns immediately (guaranteed no-op).
  - Deallocating real heap memory calls platform-aligned free (`_aligned_free` / `free`).
  - Mismatched metadata or double-free is undefined behavior.

#### `__mellis_realloc`
```c
void* __mellis_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size);
```
- **Parameters**:
  - `ptr`: Original allocation pointer (or `NULL` / sentinel).
  - `old_size`: Exact size of current allocation.
  - `old_align`: Exact alignment of current allocation.
  - `new_size`: Desired new allocation size in bytes.
- **Contract**:
  - If `ptr == NULL || is_zero_sentinel(ptr)`: Delegates to `__mellis_alloc(new_size, old_align)`.
  - If `new_size == 0`: Calls `__mellis_dealloc(ptr, old_size, old_align)` and returns `(void*)g_mellis_zero_sentinel`.
  - If `new_size > 0`: Allocates new aligned block of `new_size`, copies $\min(old\_size, new\_size)$ bytes (only if old block was not a sentinel), deallocates old block, and returns new pointer.
  - OOM failure triggers deterministic abort; old block is untouched prior to termination.

---

### 3.3 Panic & Diagnostic Traps

#### Public Primary Panic: `__mellis_panic`
```c
MELLIS_NORETURN void __mellis_panic(
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);
```
- **Contract**: Writes diagnostic to `stderr` and aborts process with exit code `101`:
  ```text
  mellis: PANIC at <file>:<line>:<col>: <message>
  ```
- Terminal `[[noreturn]]`.

#### Internal Error-Code Panic Helper: `__mellis_panic_code`
```c
MELLIS_NORETURN void __mellis_panic_code(
    uint32_t       error_code,
    const uint8_t* msg_ptr,
    size_t         msg_len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);
```
- Used internally by runtime allocator (OOM) and traps to emit standardized error codes (`[M001]`, `[M002]`, etc.) without polluting the public 9-symbol ABI.

#### Bounds Check Trap: `__mellis_bounds_fail`
```c
MELLIS_NORETURN void __mellis_bounds_fail(
    size_t         index,
    size_t         len,
    const uint8_t* file_ptr,
    size_t         file_len,
    uint32_t       line,
    uint32_t       col
);
```
- Writes out-of-bounds message to `stderr` and aborts process with exit code `102`:
  ```text
  mellis: PANIC [M002] index out of bounds: index <index>, len <len> at <file>:<line>:<col>
  ```

---

### 3.4 Primitive Unformatted Stdio

```c
void __mellis_print(const uint8_t* bytes, size_t len);
void __mellis_println(const uint8_t* bytes, size_t len);
void __mellis_eprintln(const uint8_t* bytes, size_t len);
```
- Raw unformatted byte writes.
- `println` appends `\n` to `stdout` and flushes.
- `eprintln` appends `\n` to `stderr` and flushes.

---

## 4. Summary of the Public Stable ABI Symbols

| Symbol | Signature | Owner | Role |
|:---|:---|:---:|:---|
| `__mellis_startup` | `(int argc, char** argv) -> void` | Runtime | Process state initialization |
| `__mellis_start` | `(int argc, char** argv) -> i32` | Compiler | Executable entrypoint shim |
| `__mellis_shutdown` | `(i32 exit_code) -> !` | Runtime | Process flush & exit |
| `__mellis_alloc` | `(size_t size, size_t align) -> void*` | Runtime | Infallible heap allocation |
| `__mellis_dealloc` | `(void* ptr, size_t size, size_t align) -> void` | Runtime | Explicit heap deallocation |
| `__mellis_realloc` | `(void* ptr, size_t old_size, size_t old_align, size_t new_size) -> void*` | Runtime | Infallible heap reallocation |
| `__mellis_panic` | `(msg_ptr, msg_len, file_ptr, file_len, line, col) -> !` | Runtime | Terminal panic trap |
| `__mellis_bounds_fail` | `(index, len, file_ptr, file_len, line, col) -> !` | Runtime | Terminal bounds violation trap |
| `__mellis_print` | `(bytes, len) -> void` | Runtime | Unformatted stdout write |
| `__mellis_println` | `(bytes, len) -> void` | Runtime | Unformatted stdout write + newline |
| `__mellis_eprintln` | `(bytes, len) -> void` | Runtime | Unformatted stderr write + newline |

---

## 5. Boundary Classification: Public ABI vs Internal Symbols

To ensure future ABI stability and prevent accidental leakage of implementation details, symbols are categorized strictly into two tiers:

### 5.1 Tier 1: Public Stable ABI Contract (Frozen)
The symbols in Section 4 constitute the **sole contract** between the Mellis Compiler (backend lowering) and the Mellis Standard Library (`alloc`/`std`).
- Compiler lowering and standard library bindings may **only** call these symbols.
- These signatures are guaranteed stable across runtime minor versions.

### 5.2 Tier 2: Internal Runtime Symbols (Private Implementation Details)
The following symbols are internal to `mellis-runtime` and **MUST NOT** be depended upon by compiler codegen or user libraries:
- `__mellis_panic_code`: Internal diagnostic trap supporting numeric error codes.
- `__mellis_is_zero_sentinel`: Internal sentinel pointer inspection helper for runtime unit tests.
- `__mellis_abort`: Fallback process termination primitive.
- `__mellis_argc`, `__mellis_argv`: Internal accessors for runtime startup state.
- `__mellis_free`: Backward-compatibility spelling; callers must use `__mellis_dealloc`.
- `__mellis_mem_copy`, `__mellis_mem_move`, `__mellis_mem_set`: Freestanding fallback primitives (compiler prefers LLVM intrinsics).
