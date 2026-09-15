# Freeze Audit: Stdlib-04.6 Collection Iterators (`RawTableIter`, `MapIter`, `Keys`, `Values`, `SetIter`)

**Status:** PASS — FROZEN  
**Phase:** Stdlib-04.6  
**Date:** 2026-09-14  
**Architecture Model:** Zero-allocation Sparse Traversal with Single-Provenance Lifetime Contracts and Unified View Delegation  

---

## 1. Frozen Invariants

- **ITER-1: Originating Collection Provenance**  
  Every yielded reference (`&K`, `&V`, or `(&K, &V)`) from a collection iterator is canonically provenanced to the originating collection borrow.
  `iter()`'s returned iterator provenance is derived from the input collection reference:
  $$\text{iter(self: \&Self)} \implies \text{CanonicalLifetimeContract.return\_provenance} = \text{Param}(0)$$
  Borrowck statically tracks this carried provenance back to `self`, not merely to the ephemeral iterator struct instance.

- **ITER-2: Raw Pointer Containment Boundary**  
  Raw pointers (`*u8`, `*K`, `*V`) stored in `RawTableIter` are strictly implementation state for cursor traversal. They never become a public lifetime source or escape into public API signatures.

- **ITER-3: Iterator Lifetime Bound**  
  Iterator ownership does not outlive its source collection. Attempting to store or return an iterator after the underlying `HashMap` or `HashSet` has moved or dropped is statically forbidden.

- **ITER-4: Advance Mutability Semantics (`Iterator::next(self: &rw Self)`)**  
  Advancing cursor state requires `&rw self` on the iterator instance itself. This exclusive borrow applies strictly to the iterator state machine, while yielded items carry shared provenance to the backing collection, ensuring clean decoupling between iterator iteration steps and collected references.

- **IT-1: Unified Sparse Traversal Engine (`RawTableIter<K, V>`)**  
  All collection iteration is powered by `RawTableIter<K, V>`. It directly traverses the linear state array `states: *u8`, skipping empty (`0`) and tombstone (`2`) slots with zero heap allocation and yielding only occupied (`1`) slots.
  $$\text{RawTableIter}\langle K, V\rangle \longrightarrow \text{Direct pointer arithmetic on states, keys, values}$$

- **IT-2: Zero-Cost View Wrappers Delegation**  
  `MapIter`, `Keys`, `Values`, and `SetIter` do not duplicate sparse traversal or probing logic:
  - `MapIter<K, V>` wraps `RawTableIter<K, V>` yielding `Option<(&K, &V)>`.
  - `Keys<K, V>` wraps `RawTableIter<K, V>` yielding `Option<&K>`.
  - `Values<K, V>` wraps `RawTableIter<K, V>` yielding `Option<&V>`.
  - `SetIter<T>` wraps `Keys<T, ()>` (which wraps `RawTableIter<T, ()>`) yielding `Option<&T>`.
  $$\text{SetIter}\langle T\rangle \longrightarrow \text{Keys}\langle T, ()\rangle \longrightarrow \text{RawTableIter}\langle T, ()\rangle$$

- **IT-3: LLE-v1 Single-Provenance Lifetime Elision**  
  In accordance with LLE-v1 (Luna Lifetime Elision v1):
  - `export fn iter(self: &Self) -> MapIter<K, V>`
  - `export fn keys(self: &Self) -> Keys<K, V>`
  - `export fn values(self: &Self) -> Values<K, V>`
  - `export fn iter(self: &Self) -> SetIter<T>`  
  have exactly one parameter with legal provenance (`self`). The compiler canonicalizes their lifetime contract without requiring explicit annotations in source.

- **IT-4: Borrowck Mutation Lock during Iteration**  
  Because the iterator carries provenance to `self`, attempting to mutate the collection (`insert`, `remove`, `clear`) while the iterator or any reference yielded by `.next()` is live is rejected at compile time with borrow conflict diagnostics (`E0203` / `E0204`).

- **IT-5: Robust Tombstone Skipping**  
  Traversal continues across arbitrarily deleted and re-inserted slots, visiting each currently active element exactly once without stopping early at tombstones.

- **IT-6: Self-Recursion Guard in Iterator Trait Implementations**  
  All `Iterator` trait implementations (`impl Iterator for MapIter/Keys/Values/SetIter`) forward directly to their inner engines (`self.raw.next_entry()`, `self.raw.next_key()`, `self.raw.next_value()`, `self.keys.next()`), guaranteeing zero self-referential infinite recursion.

- **IT-7: Compiler Parity & Tuple Indexing Correctness**  
  Integer tuple indexing (`pair.0`, `pair.1`, etc.) correctly extracts and evaluates field indices across complex match arms and generic structs.

- **IT-8: Source and Precompiled `.llib` Parity**  
  `alloc.ln` compiles cleanly to `alloc.llib` and `alloc.obj`. Consumers executing against precompiled `alloc.llib` exhibit identical behavior to fresh source compilation.

---

## 2. Acceptance Test Evidence

All 8 acceptance tests in [`crates/luna-driver/tests/stdlib_collection_iterators_acceptance_tests.rs`](file:///d:/fdlang/mellis-rs/crates/luna-driver/tests/stdlib_collection_iterators_acceptance_tests.rs) passed:

| Test ID | Test Name | Invariant Verified | Result |
| :--- | :--- | :--- | :--- |
| **IT-1** | `test_hashmap_iter_empty` | Empty `HashMap` iteration immediately yields `None`, clean destruction | ✅ PASS |
| **IT-2** | `test_hashmap_iter_full_traversal` | Full key-value traversal over all 5 elements, exact sum verification (`pair.0` & `pair.1`) | ✅ PASS |
| **IT-3** | `test_hashmap_iter_sparse_tombstones` | Traversal continues through multiple sparse tombstones, visiting exactly remaining elements | ✅ PASS |
| **IT-4** | `test_hashmap_keys_and_values` | Independent `keys()` and `values()` iterators yielding correct key and value streams | ✅ PASS |
| **IT-5** | `test_hashset_iter_traversal` | Full traversal of `HashSet<T>` via `SetIter<T>` wrapping `Keys<T, ()>` | ✅ PASS |
| **IT-6** | `test_hashset_iter_with_tombstones` | `HashSet<T>` iteration skipping tombstones created by set removals | ✅ PASS |
| **IT-7** | `test_borrowck_prevents_mutation_during_iteration` | Compile-time borrowck error when mutating collection while iterator is alive | ✅ PASS |
| **IT-8** | `test_iter_source_vs_llib_parity` | Identical iteration behavior between source compile and precompiled `alloc.llib` | ✅ PASS |

---

## 3. Workspace Regression Evidence

- `cargo test -p luna-driver --test stdlib_collection_iterators_acceptance_tests`: **8/8 passed**.
- `cargo test -p luna-driver --test stdlib_hashmap_acceptance_tests`: **13/13 passed**.
- `cargo test -p luna-driver --test stdlib_hashset_acceptance_tests`: **11/11 passed**.
- `cargo test -p luna-driver --test stdlib_hashmap_storage_acceptance_tests`: **8/8 passed**.
- `cargo test -p luna-driver --test stdlib_visibility_migration_tests`: **15/15 passed**.
- `cargo test --lib`: **39/39 passed** across all workspace crates.
