# Freeze Audit: Stdlib-04.4 HashMap<K, V> Public API

**Status:** PASS — READY TO FREEZE  
**Phase:** Stdlib-04.4  
**Date:** 2026-09-14  
**Architecture Model:** Thin Encapsulated Semantic Wrapper over `RawTable<K, V>` Engine  

---

## 1. Frozen Invariants

- **HM-1: Public Encapsulation and Delegation Guard (Visibility-02 & Clean Layering)**  
  `HashMap<K, V>` is the canonical user-facing associative collection in `<alloc>`. Its internal state consists exclusively of `private table: RawTable<K, V>`. All linear probing, tombstone tracking, and memory allocations are delegated to `RawTable`. No storage or probing logic is duplicated in `HashMap`.

- **HM-2: Safe Reference Contracts with `life_from(self)`**  
  Element access methods adhere to Luna canonical lifetime contracts:
  - `get(&self, key: &K) -> Option<&V> life_from(self)`
  - `get_mut(&rw self, key: &K) -> Option<&rw V> life_from(self)`  
  The returned reference's lifetime is explicitly tied to the borrow of `self`.

- **HM-3: Borrowck Verification (Read/Write Mutual Exclusion)**  
  - An active shared reference `&V` borrowed from `map.get(&k)` statically prevents any mutable access (`insert`, `get_mut`, `remove`, `clear`).
  - An active exclusive mutable reference `&rw V` borrowed from `map.get_mut(&k)` guarantees exclusive modification of the value and statically prevents concurrent reads or writes to the map.

- **HM-4: Insertion, Overwrite, and Removal Semantics**  
  - `insert(key, val)` returns `true` if inserted as a new entry, and `false` if an existing key was overwritten. Overwriting drops the old value in-place without altering key storage or length.
  - `remove(key)` removes the entry, drops the key in-place, and returns `Option::Some(val)` with the owned value, or `Option::None` if absent.
  - `clear()` drops all active key-value pairs in-place and resets length to 0 while retaining allocated capacity.

- **HM-5: Dynamic Growth & Collision Transparency**  
  - Reallocation occurs automatically when $(len + tombstones + 1) \times 4 \ge cap \times 3$.
  - All existing entries are rehashed and relocated into the new table, purging tombstones and preserving lookup fidelity across hash collisions.

- **HM-6: Destructor Soundness & Zero Leaks (DropTracker Evidence)**  
  `HashMap<K, V>` relies on compiler-generated field drop glue that invokes `Drop::drop(&rw self.table)`. DropTracker stress tests confirm:
  $$\text{Created: } 34 \quad == \quad \text{Dropped: } 34$$
  Zero memory leaks, zero double-frees, zero dangling pointers.

- **HM-7: Source and Precompiled `.llib` Parity**  
  `alloc.ln` compiles to `alloc.llib` and `alloc.obj` with identical symbol visibility, trait tables, and generic signatures. Consumers importing precompiled `alloc.llib` execute identically to source-compiled imports.

- **HM-8: Architectural Boundary Guard**  
  `HashSet<T>` is strictly decoupled and deferred to Phase 04.5. `HashMap<K, V>` contains no set-specific compromises or premature generalizations.

- **HM-9: Value Provenance & Representation Non-Escape Guard**  
  `get(&self, key)` and `get_mut(&rw self, key)` yield references strictly to the value (`&V` / `&rw V`) with canonical provenance rooted directly in `self`. They never expose internal bucket structures, raw storage pointers, slot indices, or table metadata. `RawTable` internal details strictly cannot escape into the public API surface.

---

## 2. Acceptance Test Evidence

All 13 acceptance tests in [`crates/luna-driver/tests/stdlib_hashmap_acceptance_tests.rs`](file:///d:/fdlang/mellis-rs/crates/luna-driver/tests/stdlib_hashmap_acceptance_tests.rs) passed:

| Test ID | Test Name | Invariant Verified | Result |
| :--- | :--- | :--- | :--- |
| **HM-1** | `test_hashmap_empty_lifecycle` | Default & capacity initialization (`hashmap_new`, `hashmap_with_capacity`), length, empty check, clean drop | ✅ PASS |
| **HM-2** | `test_hashmap_insert_and_get` | Basic key-value insertion, persistence across lookups, missing key handling | ✅ PASS |
| **HM-3** | `test_hashmap_contains_key` | Membership check via `contains_key` | ✅ PASS |
| **HM-4** | `test_hashmap_get_mut_inplace` | In-place element mutation via `get_mut` | ✅ PASS |
| **HM-5** | `test_hashmap_overwrite_semantics` | Key overwrite drops old value in-place, updates value, keeps length = 1 | ✅ PASS |
| **HM-6** | `test_hashmap_remove` | Removal drops key in-place, extracts value, tombstones slot | ✅ PASS |
| **HM-7** | `test_hashmap_clear` | Drops all elements, resets length to 0, preserves capacity | ✅ PASS |
| **HM-8** | `test_hashmap_multiple_collisions` | Probing and resolution across multiple colliding keys | ✅ PASS |
| **HM-9** | `test_hashmap_dynamic_growth` | Automatic growth from capacity 8 $\to$ 16 $\to$ 32, preserving all elements | ✅ PASS |
| **HM-10**| `test_hashmap_borrowck_read_blocks_mutation` | Shared borrow `&V` statically blocks mutation | ✅ PASS |
| **HM-11**| `test_hashmap_borrowck_mut_exclusive` | Exclusive mutable borrow `&rw V` blocks concurrent reads/writes | ✅ PASS |
| **HM-12**| `test_hashmap_droptracker_stress` | DropTracker: 34 created == 34 dropped across insert, overwrite, remove, resize, drop | ✅ PASS |
| **HM-13**| `test_hashmap_source_vs_llib_parity` | Parity between source compilation and precompiled `alloc.llib` | ✅ PASS |

---

## 3. Underlying Engine & Visibility Regression Evidence

- `cargo test -p luna-driver --test stdlib_hashmap_storage_acceptance_tests`: **7/7 passed** (RawTable HT-1 to HT-7).
- `cargo test -p luna-driver --test stdlib_visibility_migration_tests`: **15/15 passed** (Visibility-02 encapsulation).
- `cargo test --workspace`: **0 failures** across all crates.
