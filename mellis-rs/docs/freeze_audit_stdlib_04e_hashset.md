# Freeze Audit: Stdlib-04.5 HashSet<T> Public API

**Status:** PASS — READY TO FREEZE  
**Phase:** Stdlib-04.5  
**Date:** 2026-09-14  
**Architecture Model:** Thin Encapsulated Semantic Wrapper over `HashMap<T, ()>`  

---

## 1. Frozen Invariants

- **HS-1: Zero Duplication & Thin Layering Guard**  
  `HashSet<T>` is strictly a semantic wrapper over `HashMap<T, ()>`:
  $$\text{HashSet}\langle T\rangle \longrightarrow \text{HashMap}\langle T, ()\rangle \longrightarrow \text{RawTable}\langle T, ()\rangle \longrightarrow \text{Hash} + \text{Eq} \longrightarrow \text{generic ownership} / \text{Drop}$$
  It contains zero duplicated probing, tombstone tracking, resizing, or memory allocation logic.

- **HS-2: Visibility-02 Encapsulation**  
  The internal representation consists solely of `private map: HashMap<T, ()>`. External callers cannot inspect internal fields, read backing table arrays, or construct a `HashSet` via struct literal.

- **HS-3: Set Insert Semantics**  
  `insert(&rw self, value: T) -> bool`:
  - Returns `true` if `value` was newly added; `len` increments by 1.
  - Returns `false` if `value` was already present; the duplicate argument is deterministically dropped upon function return, the existing key in the set is preserved, and `len` remains unchanged.

- **HS-4: Set Query, Removal, and Clear Semantics**  
  - `contains(&self, value: &T) -> bool` checks membership via `Hash + Eq` without copying.
  - `remove(&rw self, value: &T) -> bool` drops the key in-place, marks the slot deleted, decrements `len`, and returns `true` if present, or `false` if absent.
  - `clear(&rw self)` drops all active elements in-place and resets `len = 0` while retaining allocated capacity.

- **HS-5: Collision Transparency & Dynamic Growth**  
  - Collision probe chains properly skip tombstones and find existing keys or first tombstones.
  - Dynamic table growth ($8 \to 16 \to 32$) rehashes and relocates all active keys without data loss or corruption.

- **HS-6: DropTracker Stress Proof: Zero Leaks & Zero Double-Frees**  
  `HashSet<T>` relies on compiler-synthesized field drop glue invoking `Drop::drop(&rw self.map)`. DropTracker stress verification confirms:
  $$\text{Created: } 18 \quad == \quad \text{Dropped: } 18$$
  Zero memory leaks, zero double-frees, zero dangling pointers across insert, duplicate rejection, removal, resize, and table destruction.

- **HS-7: Zero-Size Value Column Optimization**  
  Using `HashMap<T, ()>` utilizes 0-sized unit values `()`. Mellis runtime memory allocator (`__mellis_alloc`) assigns `g_mellis_zero_sentinel`, avoiding any redundant memory allocation for values.

- **HS-8: Decoupled Collection Iterators Boundary**  
  `HashSet<T>` v1 strictly exposes core set semantics (`insert`, `contains`, `remove`, `clear`, `len`, `capacity`, `is_empty`). Collection iterators (`HashSet::iter`, `HashMap::iter/keys/values`) are cleanly deferred to Phase 04.6.

- **HS-9: Source and Precompiled `.llib` Parity**  
  `alloc.ln` compiles cleanly to `alloc.llib` and `alloc.obj`. Consumers executing against precompiled `alloc.llib` exhibit identical behavior to fresh source compilation.

---

## 2. Acceptance Test Evidence

All 11 acceptance tests in [`crates/luna-driver/tests/stdlib_hashset_acceptance_tests.rs`](file:///d:/fdlang/mellis-rs/crates/luna-driver/tests/stdlib_hashset_acceptance_tests.rs) passed:

| Test ID | Test Name | Invariant Verified | Result |
| :--- | :--- | :--- | :--- |
| **HS-1** | `test_hashset_empty_lifecycle` | `hashset_new`, `hashset_with_capacity`, capacity, `is_empty`, clean drop | ✅ PASS |
| **HS-2** | `test_hashset_insert_and_contains` | Basic insertion, membership query via `contains`, missing key handling | ✅ PASS |
| **HS-3** | `test_hashset_duplicate_insert` | Duplicate insertion returns `false`, `len` unchanged, duplicate dropped | ✅ PASS |
| **HS-4** | `test_hashset_remove` | Value removal, in-place key drop, tombstoning, absent key handling | ✅ PASS |
| **HS-5** | `test_hashset_clear` | Drops all elements in-place, `len = 0`, capacity preserved | ✅ PASS |
| **HS-6** | `test_hashset_multiple_collisions` | Collision probing and resolution past tombstones across colliding keys | ✅ PASS |
| **HS-7** | `test_hashset_dynamic_growth` | Dynamic table growth $8 \to 16 \to 32$, all elements preserved | ✅ PASS |
| **HS-8** | `test_hashset_droptracker_stress` | DropTracker: 18 created == 18 dropped across all lifecycle events | ✅ PASS |
| **HS-9** | `test_hashset_source_vs_llib_parity` | Parity between fresh source compile and precompiled `alloc.llib` | ✅ PASS |
| **HS-10**| `test_hashset_private_field_access_rejected` | Visibility-02: external access to `set.map` rejected by compiler | ✅ PASS |
| **HS-11**| `test_hashset_struct_literal_construction_rejected` | Visibility-02: external struct literal construction rejected | ✅ PASS |

---

## 3. Underlying Collections & Workspace Regression Evidence

- `cargo test -p luna-driver --test stdlib_hashmap_acceptance_tests`: **13/13 passed** (HashMap HM-1 to HM-13).
- `cargo test -p luna-driver --test stdlib_hashmap_storage_acceptance_tests`: **7/7 passed** (RawTable HT-1 to HT-7).
- `cargo test -p luna-driver --test stdlib_visibility_migration_tests`: **15/15 passed** (Visibility-02 encapsulation).
- `cargo test --workspace`: **0 failures** across all crates.
