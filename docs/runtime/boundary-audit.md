# Mellis Runtime Boundary Audit

## 1. Executive Summary

As part of the Runtime v1.0 freeze initiative, an exhaustive audit was conducted across:
1. Compiler backend codegen (`mellis-backend/src/llvm_codegen.rs`)
2. Backend linking pipeline (`mellis-backend/src/linker.rs`)
3. Existing native runtime implementation (`d:/fdlang/runtime/`)
4. Core / Alloc library boundaries (`d:/fdlang/lib/` and `d:/fdlang/sysroot/`)

All 6 concrete blockers identified by the architecture council have been formally diagnosed and prioritized for implementation.

---

## 2. Audit Matrix: The 6 Blockers & Resolution

| ID | Domain | Issue Description | Existing Codebase State | Status | Remediation Plan |
|:---|:---|:---|:---|:---:|:---|
| **B1** | Entrypoint | Main Ownership Hazard | `llvm_codegen.rs` emits raw user `main`; `startup.c` lacks `main`. Model A would cause static archive collision. | **RESOLVED IN SPEC** | Model B: Compiler emits `@main` calling `__mellis_startup`, `__mellis_start`, and `__mellis_shutdown`. Runtime never exports `main`. |
| **B2** | Allocator ABI | `allocator.ms` realloc signature | `allocator.ms:7` has `(ptr, old_size, new_size, align)`. Mismatches SSOT. | **RESOLVED IN CODE** | Fixed to `(ptr, old_size, old_align, new_size)`. |
| **B3** | Sentinel Design | Heuristic address range | `memory.c` returned `(void*)1` and checked `ptr <= 4096`. Fails on freestanding architectures. | **RESOLVED IN SPEC** | Identity-based sentinel `g_mellis_zero_sentinel` aligned to `MELLIS_MAX_ALIGN` (4096). |
| **B4** | Allocator Safety | Alignment, OOM, Overflow, Scope bug | `memory.c` has `align` undefined in `realloc`, returns `NULL` on OOM, lacks power-of-2 validation, lacks round-up overflow check. | **READY TO PATCH** | Add validation, overflow guard, deterministic abort on OOM, fix variable scope. |
| **B5** | Panic & Bounds | C ↔ LLVM ↔ Spec signature mismatch | `panic.c` takes `(msg, len, loc)`. `llvm_codegen.rs` constructs fallback 4-param bounds fail. | **READY TO PATCH** | Standardize exact 6-parameter tuple `(msg/idx, len, file, file_len, line, col)` across C, LLVM, and FFI. Add `__mellis_panic_code`. |
| **B6** | Stdio & Codegen | Raw libc symbols in LLVM | `llvm_codegen.rs` calls `malloc`, `free`, `puts`, `printf`. `io.c` lacks `eprintln`. | **READY TO PATCH** | Add `__mellis_eprintln`. Eliminate raw libc symbols from backend codegen in favor of `__mellis_*`. |

---

## 3. Library Layering Verification

- `sysroot/lib/core`: **Verified 100% Free of Runtime Dependencies.**
  Grepped entire `sysroot` for `runtime` / `__mellis_` references: 0 occurrences.
- `lib/alloc/allocator.ms`:
  Now accurately matches the frozen runtime ABI:
  ```mellis
  export extern fn __mellis_alloc(size: uint_64, align: uint_64) -> *rw uint_8;
  export extern fn __mellis_dealloc(ptr: *rw uint_8, size: uint_64, align: uint_64);
  export extern fn __mellis_realloc(ptr: *rw uint_8, old_size: uint_64, old_align: uint_64, new_size: uint_64) -> *rw uint_8;
  ```
  Layering `core` -> `runtime ABI` -> `alloc` -> `std` is verified.
