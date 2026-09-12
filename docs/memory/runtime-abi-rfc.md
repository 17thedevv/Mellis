# Mellis Compiler RFC: Runtime Memory ABI Specification (P4 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P4 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P4)
- **Scope**: Infallible Global C Runtime ABI, FFI Symbol Signatures, Calling Conventions, Register Lowering Boundaries, ZST & OOM at ABI Boundary, Compiler Linkage, Verification
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Runtime Memory ABI ("How are allocator operations realized at the binary boundary?")

---

## 1. Architectural Purpose & Boundaries

P4 formalizes the binary transport mechanism between the compiler's semantic allocator contract (**P3**) and the native C runtime library (`mellis-runtime.lib`).

> **P4 Architectural Invariant**:
> P4 implements the contracts frozen in **P0, P1, P2, and P3**.
> P4 does **NOT** alter, weaken, or redefine any language semantics.
>
> The Runtime Memory ABI is the concrete binary realization of the global system allocator:
> $$\text{P3 GlobalAlloc} \xrightarrow{\text{P4 Lowering}} \text{Native C ABI} \xrightarrow{\text{Target Linkage}} \text{Generated Executable}$$

```text
┌────────────────────────────────────────────────────────┐
│               Mellis Compiler Middle-End               │
│  P3 Allocator Trait: Result<*rw u8, AllocError>        │
│  P3 GlobalAlloc: satisfies Allocator contract          │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ (Infallible Global Lowering)
                            ▼
┌────────────────────────────────────────────────────────┐
│            P4 Global Infallible Runtime ABI            │
│  __mellis_alloc, __mellis_dealloc, __mellis_realloc    │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ (FFI / Static Linkage)
                            ▼
┌────────────────────────────────────────────────────────┐
│            mellis-runtime.lib (C Implementation)       │
│  Platform memory allocation (_aligned_malloc / free)   │
└────────────────────────────────────────────────────────┘
```

---

## 2. Global Infallible ABI vs. Fallible Allocator Trait

To reconcile the fallible `Allocator` trait (P3) with the frozen C runtime ABI without introducing ABI bloat or breaking frozen interfaces:

1. **`__mellis_*` is the Canonical Global Infallible Runtime ABI**:
   - `__mellis_alloc`: returns non-null pointer on success; aborts deterministically on failure.
   - `__mellis_dealloc`: releases storage; returns `void`.
   - `__mellis_realloc`: returns non-null pointer on success; aborts deterministically on failure.
2. **P3 Fallible Policy Compatibility**:
   - The `Allocator` trait in P3 (`Result<*rw u8, AllocError>`) remains fallible-capable to empower custom allocators (e.g. `ArenaAllocator`, `RegionAllocator`, `PoolAllocator`) that report OOM without aborting.
   - For `GlobalAlloc`, allocation failure traps at the runtime ABI layer via `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE)`.
   - If a future requirement demands a fallible global runtime ABI, it will be introduced in a versioned RFC via a dedicated symbol (e.g. `__mellis_try_alloc`), preserving this invariant.

---

## 3. ABI Symbols & Function Signatures

All heap operations for the global allocator cross the runtime boundary via three canonical, extern "C" symbols declared in `runtime/include/mellis/runtime/memory.h`.

### 3.1 Tier 1 Public ABI Declarations (C Header)

```c
// Tier 1: Public Stable Runtime ABI (Infallible Global Allocator)
void* __mellis_alloc(size_t size, size_t align);
void  __mellis_dealloc(void* ptr, size_t size, size_t align);
void* __mellis_realloc(void* ptr, size_t old_size, size_t old_align, size_t new_size);
```

### 3.2 Mellis Language Surface Declarations

In the Mellis language runtime binding (`lib/alloc/raw.ms`):

```rust
export extern fn __mellis_alloc(size: usize, align: usize) -> *rw u8;
export extern fn __mellis_dealloc(ptr: *rw u8, size: usize, align: usize);
export extern fn __mellis_realloc(
    ptr: *rw u8,
    old_size: usize,
    old_align: usize,
    new_size: usize
) -> *rw u8;
```

---

## 4. ABI Types & Calling Conventions

### 4.1 Type Equivalence Across the Boundary

| Semantic Concept (P1–P3) | Mellis Type | C ABI Type | LLVM IR Type (x86-64) |
|:---|:---|:---|:---|
| Memory Size / Alignment | `usize` | `size_t` (`uint64_t`) | `i64` |
| Raw Untyped Storage Handle | `*rw u8` | `void*` | `ptr` (opaque pointer) |
| Infallible Execution | Guaranteed Non-Null | `void*` | `ptr` |

### 4.2 Calling Convention & Register Lowering Boundary

- **Semantic ABI Contract**: All runtime memory functions are declared as `extern "C"`.
- **Target ABI Realization**: On the current x86-64 Windows / MinGW target, LLVM lowers `extern "C"` calls according to the standard Microsoft x64 Calling Convention:
  - 32-byte shadow space provided by the caller.
  - Arguments passed in registers `RCX`, `RDX`, `R8`, `R9`.
  - Return value returned in `RAX`.

*Architectural Boundary Note*: Register mapping (`RCX`, `RDX`, etc.) is a backend/LLVM target lowering detail; it is **NOT** a semantic rule of MVIR or the Mellis language.

---

## 5. Semantic-to-ABI Mapping Contracts

### 5.1 Allocation: `allocate(layout) -> *rw u8`
- **Mapping**: Lowered to `__mellis_alloc(layout.size, layout.align)`.
- **Defensive Runtime Validation**:
  - The runtime asserts $align > 0 \land align \le \text{MELLIS\_MAX\_ALIGN} \land (align \ \& \ (align - 1)) == 0$.
  - Authoritative semantic validation is guaranteed upstream by P1 (`Layout`). The runtime assertion acts purely as defensive hardening against corrupted FFI calls.
  - The runtime never silently reinterprets an invalid alignment; it aborts immediately.
- **ZST Contract**: If $size == 0$, the runtime returns `(void*)g_mellis_zero_sentinel`. No physical OS memory is requested.
- **Infallible OOM Trap**: If the underlying platform allocator (`_aligned_malloc`) fails, the runtime invokes `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE)` and aborts. Control never returns to caller with a null pointer.

### 5.2 Deallocation: `deallocate(ptr, layout)`
- **Mapping**: Lowered to `__mellis_dealloc(ptr, layout.size, layout.align)`.
- **ZST Deallocation Contract**:
  - If $ptr == \text{g\_mellis\_zero\_sentinel}$, the runtime returns immediately with zero action.
  - *Implementation Tolerance Note*: If $ptr == \text{null}$, the runtime tolerates it as a defensive no-op. However, passing `null` is **NOT** a language-level allocator guarantee; language code must adhere to the P0/P3 deallocation preconditions.
- **Physical Release**: Invokes `_aligned_free(ptr)`.

### 5.3 Reallocation: `reallocate(ptr, old_layout, new_size) -> *rw u8`
- **Mapping**: Lowered to `__mellis_realloc(ptr, old_layout.size, old_layout.align, new_size)`.
- **Implementation Discretion**:
  `__mellis_realloc` is permitted to implement reallocation as an allocate-copy-deallocate sequence; it is not required to rely on the platform's native realloc.
- **Alignment Preservation**: The new block is guaranteed to satisfy $\text{new\_align} == \text{old\_align}$.
- **Transitions**:
  - ZST $\to$ non-ZST: If $ptr == \text{g\_mellis\_zero\_sentinel}$, delegates to `__mellis_alloc(new_size, old_align)`.
  - non-ZST $\to$ ZST: If $new\_size == 0$, invokes `__mellis_dealloc(ptr, old_size, old_align)` and returns `(void*)g_mellis_zero_sentinel`.
  - non-ZST $\to$ non-ZST:
    1. Calls `new_ptr = __mellis_alloc(new_size, old_align)`.
    2. If allocation fails, `__mellis_alloc` aborts before releasing `ptr` $\implies$ **old allocation remains fully intact, uncorrupted, and caller-owned**.
    3. Preserves overlapping raw bytes: `memcpy(new_ptr, ptr, min(old_size, new_size))` byte-for-byte.
    4. Releases old block: `__mellis_dealloc(ptr, old_size, old_align)`.
    5. Returns `new_ptr`.

---

## 6. Compiler Linkage & Backend Invariants

### 6.1 Prohibition of Raw Libc Memory Symbols (ABI-NO-LIBC)

> **ABI-NO-LIBC Invariant**:
> The Mellis compiler backend (`mellis-backend`) must **NEVER** emit direct calls or declarations for raw C libc symbols (`malloc`, `free`, `realloc`, `calloc`) into generated user modules.
>
> All memory allocation, deallocation, and reallocation must route exclusively through the canonical `__mellis_*` ABI symbols.

### 6.2 Selective Linkage Invariant
For every generated heap memory allocation operation, the compiler backend emits references exclusively to the canonical `__mellis_*` symbols. If an executable does not perform heap allocation, the linker is free to dead-strip unreferenced runtime symbols.

---

## 7. Verification & Conformance Strategy

### 7.1 Native C Test Suite (`runtime/tests/abi_tests.c`)
Automated native tests verify the C implementation against the ABI contract:
- Power-of-2 alignment validation up to 4096.
- ZST identity check: `__mellis_alloc(0, align) == sentinel`.
- `__mellis_dealloc(sentinel, 0, align)` is a no-op.
- `__mellis_realloc(sentinel, 0, align, 100)` allocates fresh buffer.
- `__mellis_realloc(buf, 100, align, 0)` releases buffer and returns sentinel.
- Infallible OOM trap test (via subprocess exit code assertion).

### 7.2 Compiler Integration Smoke Test
An end-to-end integration test in `mellis-driver` verifies:
1. Mellis program declaring external `__mellis_*` functions.
2. Allocating a 64-byte block aligned to 16 bytes.
3. Verifying pointer non-null and `ptr % 16 == 0`.
4. Writing test patterns into raw storage.
5. Reallocating to 128 bytes; asserting data preserved in $[0, 64)$ byte-for-byte.
6. **Provenance Invalidation Check**: Asserting that the old pointer is abandoned and only the new pointer is accessed.
7. Deallocating memory.
8. Allocating ZST; asserting sentinel returned and deallocated as no-op.
9. Executable runs and exits with code 0.

---

## 8. Summary & Transition Criteria to P5

With this P4 Hardened Specification:
1. The Global Infallible Runtime ABI is reconciled with the fallible P3 `Allocator` trait.
2. `__mellis_realloc` allocate-copy-deallocate semantics and alignment preservation are locked.
3. ZST deallocation contract is delineated from defensive null tolerance.
4. Selective symbol linkage is clarified.
5. Register assignment is isolated to target backend lowering.
6. Defensive runtime validation is delineated from semantic validation.
7. End-to-end provenance verification is included in the smoke test strategy.

Upon review and approval, the Tier 1 roadmap advances to **P5: MVIR Decomposed Memory Operations (`Alloc`, `Dealloc`, `Initialize`, `Write`, `MoveOut`, `DropInPlace`)**.
