# Mellis Runtime Architecture Specification v1.0

## 1. Architectural Philosophy & Layering

Mellis enforces a strict 4-layer dependency model for system libraries and execution environments:

```text
┌─────────────────────────────────────────────────────────────┐
│                            std                              │
│         (I/O, Filesystem, OS Networking, High-Level)        │
└──────────────────────────────┬──────────────────────────────┘
                               │ depends on
┌──────────────────────────────▼──────────────────────────────┐
│                           alloc                             │
│                (Box<T>, Vec<T>, String, Heap)               │
└──────────────────────────────┬──────────────────────────────┘
                               │ depends on
┌──────────────────────────────▼──────────────────────────────┐
│                     mellis-runtime ABI                      │
│       (__mellis_alloc, __mellis_panic, __mellis_startup)    │
└──────────────────────────────▲──────────────────────────────┘
                               │ (pure primitives only)
┌──────────────────────────────┴──────────────────────────────┐
│                            core                             │
│          (Option, Result, Slice, Iter, Primitives)          │
└─────────────────────────────────────────────────────────────┘
```

### Critical Layering Invariants
1. **Core Independence**: `core` **NEVER** imports or references `mellis-runtime`. `core` contains purely freestanding types, mathematical operations, and slice references. It has zero heap awareness and zero OS awareness.
2. **Runtime Isolation**: `mellis-runtime` **NEVER** depends on Mellis standard library packages (`core`, `alloc`, `std`). It is a standalone native system binary/library exporting C-compatible symbols.
3. **Alloc Dependency**: `alloc` depends on `core` for types (`Option`, `Result`) and on `mellis-runtime ABI` for raw memory management (`__mellis_alloc`, `__mellis_dealloc`, `__mellis_realloc`).
4. **Std Dependency**: `std` depends on `alloc`, `core`, and higher-level OS services surfaced via the runtime.

---

## 2. Process Lifecycle & Execution Flow (Model B: Compiler-Owned Entrypoint)

```text
       OS Kernel
           │
           │ Process spawn (argc, argv)
           ▼
┌─────────────────────────────────────────────────────────────┐
│       Compiler-Generated Entrypoint: @main(argc, argv)      │
│            (Emitted by Backend for Executables)             │
│                                                             │
│   1. call void @__mellis_startup(i32 %argc, ptr %argv)      │
│      - Captures raw OS args                                 │
│      - Configures standard handles & unbuffered I/O         │
│                                                             │
│   2. %code = call i32 @__mellis_start(i32 %argc, ptr %argv) │
│      - Calls compiler entry shim                            │
│                                                             │
│   3. call void @__mellis_shutdown(i32 %code)                │
│      - Flushes stdout/stderr buffers                        │
│      - Exits process (exit(code))                           │
│                                                             │
│   4. unreachable                                            │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│          Compiler-Generated Shim (__mellis_start)           │
│             (Emitted into user module by Backend)           │
│                                                             │
│   - If user main takes `args: [str]`:                       │
│       Allocates & converts `argv` into `[str]` slice        │
│   - Calls user `__mellis_user_main(...)`                    │
│   - Normalizes return value to `i32`                        │
│       - `void` -> `0`                                       │
│       - `i32`  -> user return code                          │
│   - Drops temporary allocations                             │
│   - Returns exit_code to caller                             │
└─────────────────────────────────────────────────────────────┘
```

### Static Linker Invariant
- **`mellis-runtime.lib` NEVER defines or exports symbol `main`**.
- The symbol `main` is emitted into the compilation unit by `mellis-backend` **only** when building an executable target.
- When compiling a library target (`.mlib` or C-FFI staticlib), the compiler does **not** emit `main` or `__mellis_start`.
- This eliminates all risk of symbol collisions, archive link order hazards, or host application entry hijacking.

---

## 3. Subsystem Breakdown

### 3.1 Startup & Lifecycle Subsystem (`startup/`)
- Exports `__mellis_startup(int argc, char** argv)`.
- Exports `MELLIS_NORETURN void __mellis_shutdown(int32_t exit_code)`.
- Does **not** export `main`.

### 3.2 Allocation Subsystem (`alloc/`)
- Implements `__mellis_alloc`, `__mellis_dealloc`, and `__mellis_realloc`.
- Enforces alignment boundary: $0 < \text{align} \le 4096$ and power of two.
- Features shared, identity-based zero-size sentinel `g_mellis_zero_sentinel`.
- Infallible: triggers immediate deterministic abort via `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE, ...)` on out-of-memory or round-up overflow.

### 3.3 Panic Subsystem (`panic/`)
- Implements `__mellis_panic` and `__mellis_bounds_fail`.
- Implements internal `__mellis_panic_code` for diagnostic tracking.
- Zero dynamic allocation: operates strictly on existing buffers and direct console file descriptors.
- Terminal `[[noreturn]]`: flushes stderr and aborts immediately.

### 3.4 Stdio Subsystem (`stdio/`)
- Implements `__mellis_print`, `__mellis_println`, `__mellis_eprintln`.
- Raw byte-level writes: zero dynamic formatting or parsing at the runtime layer.

---

## 4. Division of Responsibilities

| Responsibility | Compiler | mellis-runtime | core | alloc |
|:---|:---:|:---:|:---:|:---:|
| Lifetime & Borrow Checking | **Yes** | No | No | No |
| Drop Glue Generation | **Yes** | No | No | No |
| Bounds Comparison Logic | **Yes** | No | No | No |
| Bounds Violation Trap Handler | No | **Yes** | No | No |
| Raw Memory Allocation Block | No | **Yes** | No | No |
| `Box<T>` / `Vec<T>` Ownership | No | No | No | **Yes** |
| `Option<T>` / `Result<T, E>` | No | No | **Yes** | No |
| OS Entrypoint (`main`) | **Yes** (Executable) | **NO** | No | No |
| Entry Shim (`__mellis_start`) | **Yes** (Executable) | No | No | No |
| Startup & Shutdown Hooks | No | **Yes** | No | No |
