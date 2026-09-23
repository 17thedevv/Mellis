# Freeze Audit: Stdlib-02C Vec Correctness & IO Foundation

**Status:** SEMANTICS PASS — NATIVE E2E GATED ON COMPILER GAPS  
**Phase:** Stdlib-02C  
**Date:** 2026-09-13  
**Architecture Scope:** Stdlib only (Strict boundary guard; zero compiler code hacks)

---

## 1. Executive Verdict

```
Stdlib-02C (FROZEN / SEMANTICS PASS)
├── stdlib implementation       ✅ PASS (alloc.ln & io.ln)
├── semantic acceptance         ✅ PASS (15/15 tests in 02C suites)
├── source/.llib parity         ✅ PASS (alloc.llib & io.llib synced)
├── regression                  ✅ PASS (54/54 tests in existing suites)
├── compiler native E2E         ⚠️ GATED by existing compiler/infra gaps
└── native execution           ⏳ pending gap closure
```

---

## 2. Frozen Invariants & Narrowed Scope

### VEC-1: Element Drop Invariant
`Vec::drop` and `Vec::clear` strictly execute element destructors in reverse order (LIFO: index `len - 1` down to `0`) via `ptr::drop_in_place<T>` prior to resetting length to zero or deallocating the underlying storage buffer via `__mellis_dealloc`.

### VEC-2: Truncation Destruction Invariant
`Vec::truncate(new_len)` strictly destroys elements from index `len - 1` down to `new_len` in reverse order via `ptr::drop_in_place<T>` if and only if `new_len < self.len`. If `new_len >= self.len`, no elements are dropped and length remains unchanged.

### VEC-3: Narrowed Relocation Invariant under Move Model
> [!IMPORTANT]
> **Narrowed Invariant**:  
> *`Vec::grow` preserves exactly-once ownership for all currently legal Luna types under the current Move/Drop model.*  
> This contract is proven for the current type system (primitives, raw pointers, non-drop structs, single & nested `Box<T>`, `DropTracker`). It is explicitly **not** frozen as an unconditional theorem for arbitrary future language additions (e.g. self-referential types, pinned storage, or address-sensitive interior pointers), which must undergo independent semantic audit if ever proposed.

### VEC-4: Observation Lifetime Provenance
`Vec::last` and `Vec::last_mut` return `Option<&T>` and `Option<&rw T>` respectively, both decorated with explicit lifetime contract `life_from(self)`. Borrow checker verifies mutual exclusion, preventing mutation or re-allocation of the vector while a borrowed element reference is active.

### IO-1: Provider `<io>` vs Namespace `io::` Boundary (Rule 7)
`import <io>;` imports the IO provider artifact without polluting the caller scope. All symbols belong to `module io { ... }` and are accessed exclusively as `io::print`, `io::println`, and `io::eprintln`. Direct access to bare functions without the namespace qualifier is rejected.

### IO-2: Runtime ABI Encapsulation
Runtime extern symbols `__mellis_print`, `__mellis_println`, and `__mellis_eprintln` are **internal** to `io.ln` (declared without `export`). The public API surface is strictly composed of safe slice functions:
```rust
module io {
    export fn print(bytes: &[u8]);
    export fn println(bytes: &[u8]);
    export fn eprintln(bytes: &[u8]);
}
```
All unsafe operations (casting `bytes.data` to `*u8` and calling runtime ABI) are completely contained within provider `<io>`.

---

## 3. Compiler Capability Gaps (Preserving Architecture Boundary)

During verification of native execution for Stdlib-02C, three distinct compiler and infrastructure capability gaps were isolated and prevented from being bypassed with ad-hoc compiler hacks:

### C-GAP-01: Drop Impl Monomorphization Rooting
- **Severity**: Critical (Semantic & Codegen Correctness)
- **Subsystem**: `luna-semantic::mono::MonoCollector` (`mono.rs`), `luna-mvir::generator`, `luna-backend::llvm_codegen`
- **Root Cause**: `collect_function_roots` only enqueues `Decl::Function`. Methods inside `Decl::Impl` are only enqueued when encountering a `ConcreteToDyn` coercion. Because `typechecker.rs` (L4068) explicitly forbids direct user invocation of `val.drop()`, user struct implementations `impl Drop for T` are never enqueued into `worklist`.
- **Consequence**: Backend LLVM module contains no function definition for `fn drop()`. MVIR correctly emits `Instruction::Drop { callee: "drop", .. }`, but LLVM codegen cannot resolve the callee, skipping execution of native destructors at runtime.
- **Mandatory Chain Verification**: In the dedicated compiler gap phase, must prove the complete execution chain:
  $$\text{Drop impl} \longrightarrow \text{mono root/worklist} \longrightarrow \text{generic instantiation} \longrightarrow \text{MVIR Drop} \longrightarrow \text{resolved drop glue symbol} \longrightarrow \text{LLVM function} \longrightarrow \text{executable}$$
  Acceptance criteria must include:
  1. `Vec<Box<DropTracker>>` (grow, clear/drop $\rightarrow$ exactly $N$ destructor calls, exactly $N$ allocations freed).
  2. Nested case `Vec<Box<Box<DropTracker>>>` verifying recursive generic drop glue.

### C-GAP-02: Generic Instance Symbol Identity & Mangling
- **Severity**: Critical (Compiler Infrastructure)
- **Subsystem**: `luna-mvir::generator` (`generator.rs`)
- **Root Cause**: Generic instances are currently assigned a static `_mono` suffix (e.g. `fn_name_mono`) without discriminating parameter types.
- **Consequence**: When multiple monomorphizations of a generic helper exist in the same compilation unit (e.g. `write<i32>` and `write<Box<i32>>`), both receive identical link names (`write_mono`), triggering LLVM verification failures on parameter signature mismatch.
- **Resolution Plan**: Implement deterministic symbol mangling based on canonical semantic type representations / stable type hash (e.g. `write__T_<hash>`), without session-local ID leakage.

### C-GAP-03: External Provider Object Linking
- **Severity**: Infrastructure (Driver & Build Pipeline)
- **Subsystem**: `luna-llib::writer`, `luna-backend::linker`, `luna-driver`
- **Root Cause**: `MlibWriter` strips function bodies of non-generic library declarations in `AstInterface`. The driver linker invokes `gcc <main_obj> <runtime_lib>` but does not compile or link separate `.obj` artifacts for non-generic providers like `<io>`.
- **Consequence**: Full native binary linking for code importing non-generic library functions fails or leaves symbols unresolved, whereas AST, TypeChecker, MVIR, and Borrowck verify cleanly.
- **Resolution Plan**: Add multi-object compilation and linking in driver session for imported library providers containing non-generic object code.

---

## 4. Verification Evidence

### Test Suite Execution Summary
- `stdlib_vec_element_drop_acceptance_tests.rs`: **10 / 10 PASS**
  - `VD-1`: `Vec<Box<i32>>` scope exit drop verification
  - `VD-2`: `Vec<DropTracker>` LIFO drop order verification
  - `VD-3`: `Vec<DropTracker>::clear` drop verification
  - `VD-4`: `Vec<DropTracker>::truncate` drop verification
  - `VD-5`: `Vec<Box<DropTracker>>` multi-grow identity verification
  - `VD-6`: `Vec<i32>` primitive drop no-op
  - `VD-7`: `Vec::is_empty` correctness
  - `VD-8`: `Vec::last` / `Vec::last_mut` Option access
  - `VD-8b`: `Vec::last` borrow conflict rejection by borrowck
  - `VD-9`: `alloc.ln` vs precompiled `alloc.llib` parity
- `stdlib_io_acceptance_tests.rs`: **5 / 5 PASS**
  - `IO-0`: `io.ln` compiles to `io.llib` and syncs
  - `IO-1`: `io::println(&msg)` semantic smoke verification
  - `IO-2`: `io::print` + `io::println` sequencing verification
  - `IO-3`: Rule 7 Provider vs Namespace boundary enforcement
  - `IO-4`: `io::eprintln(&err_msg)` semantic smoke verification
- **Regression Suites**: **54 / 54 PASS**
  - `stdlib_box_ownership_acceptance_tests`: 18/18
  - `stdlib_core_ptr_mem_acceptance_tests`: 9/9
  - `stdlib_slice_borrow_acceptance_tests`: 7/7
  - `stdlib_slice_index_acceptance_tests`: 6/6
  - `stdlib_slice_iterator_acceptance_tests`: 6/6
  - `vec_iterator_lifetime_acceptance_tests`: 8/8

**Total Verified**: **69 / 69 Tests Passing**

---

## 5. Ordered Next Phase Roadmap

```
Stdlib-02C (Semantics & Stdlib Implementation PASS)
    │
    ▼
Compiler Gap Closure Phase
    ├── Fix C-GAP-01: Drop impl monomorphization rooting
    ├── Fix C-GAP-02: Deterministic generic symbol mangling
    └── Fix C-GAP-03: Provider object linking in driver session
    │
    ▼
Stdlib-02C Native E2E Verification
    └── Re-run VD-1..VD-5 & IO-1..IO-4 as native executables with runtime assertions
    │
    ▼
Stdlib-03: String / UTF-8
    └── Built cleanly on top of verified Vec, Drop, and Provider Linking
```
