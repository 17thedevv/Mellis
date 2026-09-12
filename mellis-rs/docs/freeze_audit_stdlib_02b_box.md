# Freeze Audit: Stdlib-02B Box Ownership Abstraction (Option B)

**Status:** PASS — FROZEN  
**Phase:** Stdlib-02B  
**Date:** 2026-09-13  
**Architecture Model:** Option B (Library Box Ownership Abstraction)

---

## 1. Frozen Invariants

- **BOX-1: Ordinary Library Struct**  
  `Box<T>` is an ordinary Luna struct (`struct Box<T> { ptr: *rw T }`) defined within `<alloc>` under `module std`. It is governed strictly by general language semantics (struct layout, generics, trait implementations) without compiler-level distinction.

- **BOX-2: Total Removal of Compiler Builtin Box**  
  The compiler contains no distinguished semantic Box type. `SemanticType::Box`, `CanonicalType::Box`, `Instruction::BoxFree`, `Instruction::BoxNew`, and `IntrinsicKind::BoxNew` have been completely eliminated from all compiler crates.

- **BOX-3: Generic Drop Glue Driving Destructors**  
  Destruction of payloads inside `Box<T>` is driven through generic drop glue via `ptr::drop_in_place<T>(self.ptr)` lowered directly to drop glue instruction, without custom compiler drop paths.

- **BOX-4: Strict Lifecycle Order**  
  Destruction sequence is strictly preserved:
  $$\text{DropInPlace}(T) \longrightarrow \text{Dealloc}(\text{storage})$$
  The payload destructor is executed prior to releasing heap memory.

- **BOX-5: Permanent Ownership Disarming**  
  `box_into_inner(b)` extracts the contained value and permanently disarms the Box instance by zeroing its pointer (`b.ptr = 0`) before any memory deallocation or subsequent scope exit. The Box destructor observes the disarmed state and performs no duplicate drop or deallocation.

- **BOX-6: Generic Heap Deallocation (HeapFree)**  
  Deallocation is represented in MVIR solely as generic `Instruction::HeapFree { value }` lowering to runtime `__mellis_dealloc`, decoupling compiler deallocation from any user-level Box concept.

- **BOX-7: Explicit Lifetime Provenance**  
  Safe references derived from Box (`as_ref`, `as_mut`) retain explicit lifetime provenance via `life_from(self)`, enforcing strict borrow checker mutual exclusion and preventing escape past the Box container's lifetime.

- **BOX-8: Disarmed/Null State vs Zero-Sized Types**  
  `ptr == 0` strictly represents the disarmed or null state (owning no live allocation). Zero-Sized Types (ZST) utilize the runtime's 4096-aligned non-zero sentinel (`g_mellis_zero_sentinel`), safely no-oping deallocation while preserving ownership invariants.

- **BOX-9: Source and .llib ABI Parity**  
  `<alloc>` compiles to `alloc.llib` with identical metadata, trait table entries, and generic signatures, ensuring that importing `alloc.llib` behaves identically to source compilation.

- **BOX-10: Async Heap Frame Decoupling**  
  Async transform and coroutine state machines allocate and free environment frames via raw heap primitives (`HeapAlloc` / `HeapFree`), having zero coupling or dependency on `Box<T>`.

---

## 2. Exhaustive Architectural Audit

| Subsystem | Audit Query / Check | Result | Evidence |
| :--- | :--- | :--- | :--- |
| **Semantic Types** | `SemanticType::Box` | 0 matches | Removed from `ty.rs`, `typechecker.rs`, `mono.rs`, `coherence.rs` |
| **LLib Metadata** | `CanonicalType::Box` | 0 matches | Removed from `metadata.rs`, `metadata_builder.rs`, `metadata_decoder.rs` |
| **MVIR Instructions** | `Instruction::BoxFree` | 0 matches | Replaced by generic `Instruction::HeapFree` |
| **MVIR Instructions** | `Instruction::BoxNew` | 0 matches | Removed across `mvir.rs`, `printer.rs`, `interp.rs`, `dce.rs`, `verifier.rs` |
| **Semantic Intrinsics**| `IntrinsicKind::BoxNew`| 0 matches | Removed from `semantic_tables.rs` |
| **Comptime Types** | `PointerKind::Box` | 0 matches | Removed from `comptime/type_repr.rs` |
| **Borrow Checker** | Box-specific branches | 0 matches | `borrow_analysis.rs` relies entirely on generic struct borrow analysis |
| **Backend Lowering** | LLVM Box handlers | 0 matches | `llvm_codegen.rs` lowers only generic `HeapFree` to `__mellis_dealloc` |

---

## 3. Acceptance Proofs (B1–B18)

Automated tests in `crates/luna-driver/tests/stdlib_box_ownership_acceptance_tests.rs`:
- **B1**: `test_b1_box_new_allocation_and_init` — PASS
- **B2**: `test_b2_box_payload_drop_glue` — PASS
- **B3**: `test_b3_box_into_inner_consuming_move` — PASS
- **B4**: `test_b4_box_lifecycle_order` — PASS
- **B5**: `test_b5_box_use_after_move_rejected` — PASS
- **B6**: `test_b6_box_borrow_conflict_rejection` — PASS
- **B7**: `test_b7_disarmed_sentinel_vs_live` — PASS
- **B8**: `test_b8_box_zero_sized_type` — PASS
- **B9**: `test_b9_box_dealloc_not_in_public_std` — PASS
- **B10**: `test_b10_box_struct_defined_in_module_std` — PASS
- **B11**: `test_b11_consumer_import_alloc_accesses_std_box` — PASS
- **B12**: `test_b12_rule_7_provider_vs_namespace` — PASS
- **B13**: `test_b13_source_vs_llib_parity` — PASS
- **B14**: `test_b14_no_box_compiler_special_cases` — PASS
- **B15**: `test_b15_generic_monomorphization_multiple_types` — PASS
- **B16**: `test_b16_nested_box_ownership` — PASS
- **B17**: `test_b17_box_where_t_implements_drop` — PASS
- **B18**: `test_b18_nested_box_scope_destruction` — PASS

## 4. Verification Suite Pass
- `cargo test --workspace -- --test-threads=1`: 100% PASS, 0 failures across all compiler crates.
- Integration tests in `alloc_box_tests.rs`: 9/9 PASS.
- Integration tests in `stdlib_slice_index_acceptance_tests.rs`: 6/6 PASS.
- Integration tests in `stdlib_slice_borrow_acceptance_tests.rs`: 7/7 PASS.
