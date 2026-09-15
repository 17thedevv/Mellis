# Freeze Audit: Stdlib-04.3 RawTable Storage Engine

**Status:** PASS — READY TO FREEZE  
**Phase:** Stdlib-04.3  
**Date:** 2026-09-14  
**Architecture Model:** Disjoint Parallel Raw Arrays with Linear Probing and First-Tombstone Reuse  

---

## 1. Frozen Invariants

- **RT-1: Disjoint Parallel Storage (BOX-FROZEN Compliance)**  
  `RawTable<K, V>` completely avoids any value-typed `Bucket<K, V>` struct with uninitialized fields. Storage is maintained via 4 parallel disjoint allocations:
  - `states: *rw u8` (slot state tags: 0 = Empty, 1 = Occupied, 2 = Deleted)
  - `hashes: *rw u64` (full 64-bit cached hashes)
  - `keys: *rw K` (raw memory; only initialized when state == Occupied)
  - `values: *rw V` (raw memory; only initialized when state == Occupied)
  Compiler drop glue is never invoked on uninitialized memory.

- **RT-2: Probing Algorithm & Standardized Terminology**  
  The collision resolution algorithm is strictly standardized as:  
  $$\textbf{Linear probing with first-tombstone reuse}$$  
  Probe formula: $\text{index}(i) = (\text{hash} + i) \ \& \ (\text{capacity} - 1)$.

- **RT-3: Three-State State Machine**  
  - $\text{Empty (0)}$: Slot is vacant; terminates forward search probing.
  - $\text{Occupied (1)}$: Slot owns an active, live $K$ and $V$ pair.
  - $\text{Deleted (2)}$: Slot is a tombstone; search probing continues forward.

- **RT-4: Search Correctness Across Tombstones**  
  - If a tombstone is encountered during probing, the index is recorded as `first_tombstone` and search proceeds forward.
  - If a matching key is found later in the probe sequence, `find` **MUST** return `ProbeResult::Found(idx)` and **MUST NOT** prematurely return `ProbeResult::Vacant`.
  - If search terminates upon reaching an `Empty` slot, `find` returns `ProbeResult::Vacant(first_tombstone)` if a tombstone was encountered, or `ProbeResult::Vacant(empty_idx)` otherwise.

- **RT-5: Overwrite Semantics**  
  Inserting an existing key executes in-place destruction of the old value via `ptr::drop_in_place<V>`, writes the new value via `ptr::write<V>`, and leaves key storage and table length unchanged.

- **RT-6: Removal Semantics**  
  Removing an entry executes in-place destruction of the key via `ptr::drop_in_place<K>`, extracts the value via `ptr::read<V>`, sets state to `Deleted (2)`, decrements `len`, and increments `tombstones`.

- **RT-7: Growth and Tombstone Purging**  
  When load threshold is exceeded ($(\text{len} + \text{tombstones} + 1) \times 4 \ge \text{capacity} \times 3$), `grow()` allocates new capacity ($2 \times \text{cap}$), moves only `Occupied` elements via `ptr::read` and `ptr::write`, deallocates old memory without triggering drop glue on moved elements, and resets `tombstones = 0`.

- **RT-8: Destructor Safety & Zero Leaks**  
  `impl<K, V> Drop for RawTable<K, V>` iterates through the table and calls `ptr::drop_in_place` strictly on slots where `state == Occupied (1)`. Empty and Deleted slots are untouched. All 4 raw buffers are freed via `__mellis_dealloc`.

- **RT-9: Internal Storage Engine Encapsulation**  
  `RawTable<K, V>` is an internal storage engine specifically designed to power `HashMap<K, V>` and `HashSet<T>`. All 7 representation fields are `private`. It is not part of the frozen public user-facing API surface.

- **RT-10: Source and .llib Parity**  
  `alloc.ln` compiles to `alloc.llib` with identical behavior and layout, proven by end-to-end execution against precompiled artifacts.

---

## 2. Acceptance Test Evidence

All acceptance tests in [`crates/luna-driver/tests/stdlib_hashmap_storage_acceptance_tests.rs`](file:///d:/fdlang/mellis-rs/crates/luna-driver/tests/stdlib_hashmap_storage_acceptance_tests.rs) passed:

| Test ID | Test Name | Invariant Verified | Result |
| :--- | :--- | :--- | :--- |
| **HT-1** | `test_ht1_empty_lifecycle` | Empty table initialization, capacity, zero tombstones, clean drop | ✅ PASS |
| **HT-2** | `test_ht2_insert_persistence_and_contains` | Basic insertion, persistence across lookups, `contains_key`, missing key | ✅ PASS |
| **HT-2B**| `test_ht2b_get_mut` | In-place mutation through mutable reference `get_mut` | ✅ PASS |
| **HT-3** | `test_ht3_overwrite_semantics` | In-place overwrite drops old value, keeps length = 1 | ✅ PASS |
| **HT-4** | `test_ht4_tombstone_search_correctness_and_reuse` | Collision chain probing past tombstones, existing key returns `Found`, new key reuses tombstone | ✅ PASS |
| **HT-5** | `test_ht5_resize_growth_and_relocation` | Resizing 8 $\to$ 16 $\to$ 32, all elements preserved, tombstones cleared | ✅ PASS |
| **HT-6** | `test_ht6_drop_tracker_stress_soundness` | DropTracker: 34 created == 34 dropped across all operations (zero leaks, zero double-frees) | ✅ PASS |
| **HT-7** | `test_ht7_source_vs_llib_parity` | Execution parity between source and precompiled `alloc.llib` | ✅ PASS |

---

## 3. Workspace Regression Verification

- `cargo test --workspace`: **0 failures** across all crates.
- `cargo test -p luna-driver --test stdlib_visibility_migration_tests`: **15/15 passed** (Visibility-02 compliance).
