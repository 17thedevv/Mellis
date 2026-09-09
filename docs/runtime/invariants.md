# Mellis Runtime Normative Invariants v1.0 (FROZEN)

## 1. Table of Normative Invariants

| ID | Domain | Invariant Title | Severity |
|:---|:---|:---|:---:|
| **INV-RT-1** | Entrypoint | Model B Entrypoint & Shim Contract | **CRITICAL** |
| **INV-RT-2** | Memory | Layout Metadata Symmetry | **CRITICAL** |
| **INV-RT-3** | Memory | Deterministic OOM Panic (No Silent NULL) | **CRITICAL** |
| **INV-RT-4** | Memory | Non-Zero Power-of-2 Alignment ($0 < \text{align} \le 4096$) | **CRITICAL** |
| **INV-RT-5** | Memory | Identity-Based Shared Zero-Size Sentinel | **CRITICAL** |
| **INV-RT-6** | Panic | Terminal Abort (No Unwinding) | **CRITICAL** |
| **INV-RT-7** | Bounds | Deterministic Bounds Violation Trap | **CRITICAL** |
| **INV-RT-8** | Stdio | Primitive Unformatted Byte Writing | **MAJOR** |
| **INV-RT-9** | Symbol | Strict `__mellis_` Namespace Boundary | **CRITICAL** |
| **INV-RT-10**| Layering | Core Runtime Independence | **CRITICAL** |

---

## 2. Invariant Specifications

### INV-RT-1: Model B Entrypoint & Shim Contract
- **Rule**:
  1. The static library `mellis-runtime.lib` **never** defines or exports `main`. It exports `__mellis_startup(int, char**)` and `__mellis_shutdown(int32_t)`.
  2. The compiler backend emits `@main(i32 %argc, ptr %argv) -> i32` **only** when generating an executable target.
  3. `@main` calls `@__mellis_startup`, invokes `@__mellis_start`, calls `@__mellis_shutdown`, and is followed by LLVM `unreachable`.
  4. The compiler backend emits `@__mellis_start` to bridge OS arguments to user `__mellis_user_main(...)` and normalize exit code to `i32`.
  5. Library targets (`.mlib`) emit neither `main` nor `__mellis_start`.

### INV-RT-2: Layout Metadata Symmetry
- **Rule**: Deallocation via `__mellis_dealloc(ptr, size, align)` and reallocation via `__mellis_realloc(ptr, old_size, old_align, new_size)` must receive the exact `size` and `align` corresponding to the original allocation.
- **Requirements**:
  1. The caller (compiler drop glue or standard library allocator) owns layout metadata.
  2. The runtime does not store hidden header metadata in memory chunks to "rescue" or guess layout.
  3. Calling `dealloc` with mismatched size or alignment is undefined behavior.

### INV-RT-3: Deterministic OOM Panic (No Silent NULL)
- **Rule**: In Mellis v1.0, memory allocation failure (OOM) must **never** return a silent `NULL` pointer to user code.
- **Requirements**:
  1. If the underlying OS or hardware allocator fails to satisfy `__mellis_alloc` or `__mellis_realloc`, the runtime must immediately invoke `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE, ...)` and abort execution.
  2. Fallible allocation APIs (e.g. `try_alloc`) are out-of-scope for v1.0 and deferred until language support for `Result`/`Option` allocators is ratified.

### INV-RT-4: Alignment Boundary ($0 < \text{align} \le 4096$)
- **Rule**: Every `align` parameter passed to `__mellis_alloc`, `__mellis_dealloc`, or `__mellis_realloc` must satisfy:
  $$0 < \text{align} \le 4096 \quad \land \quad (\text{align} \mathrel{\&} (\text{align} - 1)) == 0$$
- **Requirements**:
  1. If `align == 0`, `align > 4096`, or `align` is not a power of 2, the runtime triggers an immediate abort (`MELLIS_ERR_INVALID_STATE`).
  2. Returned heap pointers $P$ must satisfy $P \pmod{\text{align}} == 0$.

### INV-RT-5: Identity-Based Shared Zero-Size Sentinel
- **Rule**: Allocating zero bytes (`size == 0`) returns a pointer to a single, runtime-owned sentinel object aligned to `MELLIS_MAX_ALIGN` (4096).
- **Requirements**:
  1. Sentinel recognition is strictly identity-based:
     $$\text{is\_zero\_sentinel}(P) \iff P == (\text{const void*})\&g\_mellis\_zero\_sentinel$$
  2. The runtime never infers sentinel status from arbitrary address ranges (e.g. `ptr <= 4096`), ensuring identical semantics on hosted, freestanding, and embedded architectures.
  3. `__mellis_dealloc(ptr, size, align)` where `is_zero_sentinel(ptr)` is true is a guaranteed no-op.
  4. Dereferencing the sentinel is forbidden by safe language semantics.

### INV-RT-6: Terminal Abort (No Unwinding)
- **Rule**: Panic is an unrecoverable, terminal event in Mellis v1.0.
- **Requirements**:
  1. `__mellis_panic`, `__mellis_panic_code`, `__mellis_bounds_fail`, and `__mellis_shutdown` are marked `[[noreturn]]`.
  2. Execution never returns to the caller or unwinds frames in v1.0.
  3. All open diagnostic streams (`stderr`, `stdout`) are flushed prior to process termination.

### INV-RT-7: Deterministic Bounds Violation Trap
- **Rule**: Every array, slice, or collection index violation that escapes compiler optimization must terminate via `__mellis_bounds_fail`.
- **Requirements**:
  1. Emits standard diagnostic: `mellis: PANIC [M002] index out of bounds: index <index>, len <len> at <file>:<line>:<col>`.
  2. Immediately terminates with exit code `102`.

### INV-RT-8: Primitive Unformatted Stdio
- **Rule**: `__mellis_print`, `__mellis_println`, and `__mellis_eprintln` operate on raw, unformatted byte sequences (`const uint8_t*, size_t`).
- **Requirements**:
  1. Zero formatting interpretation (no `%d`, no `{}`).
  2. Zero dynamic memory allocation.
  3. `println` and `eprintln` guarantee inclusion of trailing `\n`.

### INV-RT-9: Strict `__mellis_` Namespace Boundary
- **Rule**: Public runtime ABI symbols must strictly match `^__mellis_[a-z0-9_]+$`.
- **Requirements**:
  1. The compiler backend must **not** emit direct calls to external C library symbols (`malloc`, `free`, `puts`, `printf`, `exit`).
  2. All memory, panic, and stdio operations lowered from MVIR must call the corresponding `__mellis_*` ABI symbol.

### INV-RT-10: Core Runtime Independence
- **Rule**: The Mellis `core` library must have zero dependencies on `mellis-runtime`.
- **Requirements**:
  1. No import of runtime symbols or hooks from `core`.
  2. `core` must remain 100% buildable and usable in freestanding environments without any runtime binary linked.
