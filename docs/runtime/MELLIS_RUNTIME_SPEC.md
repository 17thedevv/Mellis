# Mellis Runtime Architecture and ABI Specification v1.0

## 1. Executive Summary
This document serves as the authoritative source of truth for the Mellis Runtime Architecture and Application Binary Interface (ABI) version 1.0. It defines the exact boundary between compiler-generated code (MVIR/LLVM IR) and runtime library services. It provides the architectural blueprint for implementing the Mellis runtime across Hosted, Freestanding, and Embedded targets without duplicating language semantics or prematurely committing to specific OS abstractions.

## 2. Scope
This specification covers:
- The `mellis-runtime` architectural layout.
- The ABI for memory allocation, panic handling, startup, closures, and concurrency primitives.
- The separation of responsibilities between Compiler, Runtime, and Standard Library.
- Platform profiles (Hosted vs Freestanding vs Embedded).
- Linkage, packaging, versioning, and testing expectations.

## 3. Terminology
- **Mellis Compiler:** Translates source (`.ms`) into MVIR, enforcing ownership, lifetime, and semantic rules.
- **MVIR (Mellis Validator IR):** The explicit mid-level representation of Mellis semantics.
- **LLVM Backend:** Lowers MVIR into target machine representation.
- **Mellis Runtime:** Implements low-level operations that cannot or should not be generated entirely in machine code.
- **Mellis Standard Library (`std`, `alloc`, `core`):** Programmer-facing APIs built on runtime primitives.

## 4. Runtime Responsibilities
**Responsibilities:**
- OS/Platform boundary interface (Program entry/exit, arguments, environment).
- Primitive allocation/deallocation routing (calling `malloc`/`free` or custom allocators).
- Trap/panic payload routing and immediate process abort/unwinding handling.
- Foundational abstractions for thread creation and synchronization (Mutex/Yield) to shield `std`.
- Resolution of escaping heap closures and their environment management.

**Non-responsibilities:**
- The runtime does **NOT** enforce ownership, lifetimes, or borrow rules (this is done in MVIR).
- The runtime does **NOT** contain a Garbage Collector (GC), ARC implicitly, or package manager.
- The runtime is **NOT** a utility standard library (e.g., no string manipulation, filesystem abstractions, or collections).
- The runtime must **NOT** silently mask undefined behavior.

## 5. Architecture
The runtime is layered into distinct subsystems:
```text
mellis-runtime/
├── abi/         # ABI headers and stable external declarations
├── core/        # Common utilities used internally by the runtime
├── memory/      # Allocation primitives
├── panic/       # Trap, bounds check, and assertion handlers
├── process/     # Exit, abort, environment variables
├── threading/   # Low-level thread spawning and identity
├── sync/        # Atomic, Mutex, Condvar OS bindings
├── coroutine/   # Frame allocation, wake/suspend hooks
├── platform/    # OS-specific (windows, linux, macos, freestanding)
├── ffi/         # Calling convention and C-boundary utilities
└── startup/     # Entry points (CRT -> Mellis main)
```

## 6. ABI (Application Binary Interface)
All runtime symbols are prefixed with `__mellis_` to prevent namespace collisions. The Runtime ABI uses a C-compatible external calling convention (`extern "C"`) for portability. (Note: Mellis ABI and C FFI ABI are distinct; the runtime simply uses the C ABI for its own interface). Functions adhere to strict architectural sizing (e.g., `size_t`).

## 7. Memory Management
The Memory ABI is platform-independent. In a hosted profile, it maps to `malloc`/`free`. In a freestanding profile, it requires user implementation.

### Primitives
- **Compiler Intrinsics**: `memcpy`, `memmove`, `memset` are primarily lowered directly to LLVM intrinsics.
- **Optional Runtime Fallbacks**: `__mellis_mem_copy`, `__mellis_mem_move`, `__mellis_mem_set` may be provided for environments lacking built-in memory operations. The runtime is not strictly forced to expose all standard memcmp-like operations.

### Allocation Semantics
- **Pointer validity:** Returns non-null on success. Always returns `null` on out-of-memory failure. The runtime primitive is predictable and low-level; panic policies on OOM are strictly delegated to higher-level abstractions like `Box` or `Vec` in `alloc`.
- **Zero-size allocation:** May return a dangling, non-null, uniquely aligned pointer (e.g., `0x1`).
- **Deallocation:** `__mellis_dealloc` requires `size` and `align` for compatibility with custom, debug, or embedded allocators, even though hosted default allocators (like libc `free`) may ignore them.
- **Ownership:** Caller takes ownership of the allocated memory.

## 8. Panic / Trap System
Provides a unified failure mechanism without assuming exceptions/unwinding.

- **Abort vs Unwind:** By default, panic triggers an abort. Unwinding is purely optional and disabled in freestanding profiles.
- **Exit Status:** Non-zero exit code upon panic.

## 9. Drop / Destruction
**Compiler-Generated Destruction:** The compiler implicitly generates code to drop fields and captures.
**Runtime Role:** The runtime is strictly responsible for *storage reclamation* (via `__mellis_dealloc`), *never* logical object destruction. 
The runtime must NOT blindly free memory without the compiler first executing the generated drop glue.

## 10. Closure Runtime Support
- **Environment ABI:** Closures that escape are allocated on the heap via `__mellis_alloc`. The environment is a compiler-defined anonymous struct.
- **Destruction:** The compiler generates a destructor function that destroys captures, then calls `__mellis_dealloc` for the environment pointer.
- **Zero-capture closures:** Degrade to standard function pointers; no runtime allocation occurs.

## 11. Trait Objects / VTable Support
VTables are **statically generated** by the compiler as constant data.
The runtime itself does **NOT** participate in dynamic dispatch. It has no knowledge of "Trait Objects", `Box<T>`, or closures. It only sees memory pointers, size, and alignment.

## 12. Coroutine / Async Support
**Coroutine ABI ≠ Async Language Semantics ≠ Executor API.**
- **Compiler**: Handles coroutine lowering and suspend points.
- **Runtime ABI**: Handles frame allocation (`__mellis_coroutine_alloc`) and destruction (`__mellis_coroutine_free`).
- **Std/Library**: Provides the Async Executor. The core runtime does not dictate or include an async executor.

## 13. Threading
The runtime provides **low-level OS/thread ABI bindings**, not foundational public abstractions.
- `__mellis_thread_spawn`, `__mellis_thread_join`, `__mellis_thread_yield`, `__mellis_thread_sleep`.
These are backend primitives for `std::thread`. The public thread API belongs entirely in `std`.

## 14. Synchronization
The runtime provides **native mutex ABI bindings**:
- `__mellis_mutex_create`, `__mellis_mutex_lock`, `__mellis_mutex_unlock`
- `__mellis_condvar_wait`, `__mellis_condvar_signal`
The public `Mutex` and `Condvar` data structures belong in `std`.
*Note:* Standard atomic operations are generated purely via LLVM intrinsics; the runtime only provides locking fallbacks for platforms lacking atomic instructions.

## 15. Global / Static Initialization
- Constant initialization is done at compile-time (BSS/Data sections).
- Runtime initialization of globals (if supported) is managed by `__mellis_startup`, executing registered initialization routines in dependency order before `main`.

## 16. Program Startup
Program startup is abstracted to support CRT, bare-metal, and custom entries:
```text
Platform Entry (CRT / _start / WinMain)
    ↓
Mellis Runtime Bootstrap
    ↓
Global Statics Init
    ↓
Mellis main (User main.ms)
    ↓
Shutdown
    ↓
Platform Exit
```
Note that `__mellis_startup` may not have the exact same ABI across all profiles (e.g., bare-metal vs hosted Windows).

## 17. FFI Boundaries
The Runtime guarantees transparent mapping of Mellis types to the System V / Windows ABI where specified via `extern fn`.
- The runtime does not intercept FFI calls.
- Panic crossing an FFI boundary immediately aborts the process to prevent undefined behavior in foreign code.

## 18. Platform Abstraction
`runtime/platform/` contains:
- `windows/` (Win32 API)
- `linux/` (Syscalls/glibc)
- `macos/` (Darwin APIs)
- `freestanding/` (No-OS)
- `embedded/` (Baremetal Cortex-M, RISC-V)

## 19. Hosted / Freestanding / Embedded
- **Hosted:** Full allocator, OS threads, filesystem boundaries.
- **Freestanding:** No OS assumptions. Minimal panic trap. Missing thread/sync primitives.
- **Embedded:** No mandatory libc. Linker script compatible. Custom allocator support.

## 20. Packaging
Installation layout:
```text
mellis/
├── bin/
├── runtime/
│   ├── hosted/
│   ├── freestanding/
│   └── embedded/
├── lib/
│   ├── core/
│   ├── alloc/
│   └── std/
└── toolchain/
```
The compiler automatically resolves and links the correct runtime target. Users run `mellis main.ms` without manual `-l mellis-runtime` flags.

## 21. Versioning
- **ABI Version:** Currently `v1`. Encoded in symbol names or metadata section.
- The compiler strictly verifies that the linked runtime ABI matches the compiler's expected ABI.

## 22. Compiler Integration
The compiler lowers high-level constructs directly to runtime calls during LLVM generation:
- `HeapAllocInst` → `__mellis_alloc`
- `HeapFreeInst` → `__mellis_dealloc`
- Bounds check fail → `__mellis_bounds_fail`

## 23. Standard Library Boundary
- **`core`**: Architecturally independent. Includes `Option`, `Result`, `Slice`, primitives. No runtime allocation.
- **`alloc`**: Depends on runtime memory allocator. Includes `Box`, `Vec`, `String`.
- **`std`**: Depends on OS/Runtime process bindings. Includes `File`, `Socket`, `Thread`.
- **`runtime`**: Actual OS bindings and minimal symbols (`__mellis_alloc`, `__mellis_panic`).

## 24. Testing Strategy
- Positive, negative, and stress tests located in `tests/runtime/`.
- **ABI Conformance Tests:** Compiler tests ensuring generated LLVM calls match the exact C-signature of `__mellis_*` symbols.

## 25. Security Considerations
Unsafe operations remain explicitly unsafe at the language level. The runtime never silently patches data races or invalid ownership. Invalid deallocation (e.g., double-free) is outside the safe language semantics and may result in process termination or undefined native behavior. Debug runtime may detect some allocator misuse, but this is not a language guarantee.

## 26. Performance Constraints
- Zero-cost where possible.
- Primitives (math, logic) are intrinsic, not runtime calls.
- Allocator fast-paths must be inlineable.

## 27. Bootstrap Strategy
1. **Stage 0:** C/ASM bootstrap runtime.
2. **Stage 1:** Mellis compiler (built via C++).
3. **Stage 2:** Mellis runtime compilation.
4. **Stage 3:** Mellis `core`, `alloc`, `std`.

## 28. Future Extensions
- Thread-local storage (TLS) via `__mellis_tls_*`.
- Stack unwinding payloads for catchable panics.
- Advanced memory profiling hooks.

## 29. Non-goals
- Adding an ARC runtime or Garbage Collector.
- Replacing libc wholesale (Mellis will dynamically link libc in hosted environments).
- Adding networking/HTTP stacks to the core runtime (belongs in `std` or ecosystem).

## 30. Open Design Decisions
- Should `panic` payloads carry formatted strings or just static error IDs in embedded targets?
- Should default `__mellis_alloc` fallback to a bundled rpmalloc, or strictly `malloc`?

---

# Appendices

### Appendix A: Runtime ABI Table

| Function | Parameters | Return | Semantics / Failure |
|----------|------------|--------|---------------------|
| `__mellis_alloc` | `size_t size`, `size_t align` | `void*` | Allocates memory. Returns `null` on OOM. |
| `__mellis_dealloc` | `void* ptr`, `size_t size`, `size_t align` | `void` | Frees memory. Hosted default allocators may ignore size/align. Invalid pointer = UB. |
| `__mellis_realloc` | `void* ptr`, `size_t old_size`, `size_t new_size`, `size_t align` | `void*`| Reallocates. Returns `null` on OOM. |
| `__mellis_panic` | `PanicInfo* info` | `! (Never)` | Aborts execution. `PanicInfo` contains message, file, line, column, and error code. |
| `__mellis_bounds_fail`| `size_t idx`, `size_t len`, `const char* file`, `int line` | `! (Never)` | Array/slice bounds check failure. |
| `__mellis_startup` | `int argc`, `char** argv` (Platform dependent) | `void` | Initializes runtime context. |

### Appendix B: Responsibility Matrix

| Feature / Abstraction | `core` | `alloc` | `std` | `runtime` |
|-----------------------|--------|---------|-------|-----------|
| `Option` / `Result`   | Yes    | No      | No    | No        |
| `Vec<T>`, `String`    | No     | Yes     | No    | No        |
| `File` / `Socket`     | No     | No      | Yes   | No        |
| `Box<T>`              | No     | Yes     | No    | No        |
| Raw Allocator C-ABI   | No     | No      | No    | Yes       |
| Bounds check logic    | No     | No      | No    | Yes (hook)|
| Public Mutex API/Type | No     | No      | Yes   | No        |
| Native Mutex ABI      | No     | No      | No    | Yes       |

### Appendix C: Compiler Flow Diagram

```text
[ Compiler (MVIR -> LLVM) ]           [ core / alloc / std ]
             │                                   │
             └──────────────┐     ┌──────────────┘
                            ↓     ↓
                     [ Runtime ABI Contract ]
                               ↓
                   [ Runtime Implementation ]
                               ↓
                 [ Platform / OS / Hardware ]
```

### Appendix D: Implementation Milestone Plan

1. **Milestone 1:** Memory & Panic Foundations (`__mellis_alloc`, `__mellis_panic`).
2. **Milestone 2:** Process Startup & Shutdown (`__mellis_startup`, args parsing).
3. **Milestone 3:** Threading & Sync bindings (OS wrapping).
4. **Milestone 4:** FFI and Freestanding stubs.
5. **Milestone 5:** CI ABI Conformance testing integration.

### Appendix E: Unresolved Decisions Before Implementation

1. **Freestanding Initialization:** How is the `.bss` and `.data` setup triggered in bare-metal deployments without relying on a C runtime (`crt0`)?
2. **Runtime Thread Identity:** Should thread-local unique IDs be maintained by the OS, or cached in userspace by `mellis-runtime`?
