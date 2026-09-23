# Freeze Audit: Stdlib-05.3 Iterator Subsystem — Collection Transformers (`collect`)

## 1. Executive Summary

- **Phase**: Stdlib-05.3
- **Scope**: Collection Transformers (`iter_collect_vec`, `iter_collect_hashset`, `iter_collect_hashmap`)
- **Status**: **FROZEN**
- **Compiler Changes**: **NONE** (Zero modifications to `crates/luna-*` compiler crates; strictly compliant with Rule 11)
- **Artifacts**:
  - `libs/external/alloc.ln` (implemented 3 free functions)
  - `libs/external/alloc.llib` (recompiled and synced)
  - `crates/luna-driver/tests/stdlib_iterator_collect_acceptance_tests.rs` (15 acceptance tests C1–C15)
- **Regression**: 15/15 Stdlib-05.3 passed, full iterator and collection suites green.

---

## 2. Invariants Frozen

### COLLECT-1: Collection Target Invariance
`collect` is exposed exclusively through 3 canonical free functions:
```rust
export fn iter_collect_vec<I: Iterator<Item>, Item>(iter: I) -> Vec<Item>;
export fn iter_collect_hashset<I: Iterator<Item>, Item: Hash + Eq>(iter: I) -> HashSet<Item>;
export fn iter_collect_hashmap<I: Iterator<(K, V)>, K: Hash + Eq, V>(iter: I) -> HashMap<K, V>;
```
- Short generic parameter names `I, Item, K, V` strictly preserved.
- No `FromIterator` trait introduced (deferred per architecture consensus).
- Zero compiler intrinsics or opcode dispatch.

### COLLECT-2: Allocation-Failure & Partial Construction Safety
> *A partially constructed collection owns and drops every successfully inserted element exactly once when its owner is destroyed.*
- In `iter_collect_*`, collection accumulation uses local mutable bindings (`vec`, `set`, `map`).
- If an iterator terminates or panics mid-stream, the collection's drop glue correctly traverses all initialized slots, ensuring zero leaks and zero double-drops.

### COLLECT-3: Set/Map Key Uniqueness and Ownership
- `iter_collect_hashset`: Duplicate items are rejected per HS-3. The incoming duplicate item is discarded and dropped, preserving the original set element.
- `iter_collect_hashmap`: Duplicate keys trigger last-wins value replacement per HM-4. The incoming key is dropped, the old value is dropped in-place, and the incoming value overwrites the slot while preserving the original key.
- Verified with custom `TrackedKey`/`TrackedVal` identity counters and marker assertions.

### COLLECT-4: Borrowed Iterator Modality & Ownership
- `SliceIter<T>` yields `Item = &T`.
- Collections (`Vec<T>`, `HashSet<T>`, `HashMap<K, V>`) own their elements.
- Attempting to store `&T` references in an owned container without valid lifetime provenance is caught by Borrowck (`E3005`). Collecting from borrowed sequences into owned collections canonically maps through value dereference (`iter_map(slice_iter(&arr), deref)`).

### COLLECT-5: Empty Stream Safety
- Collecting an empty iterator produces a valid collection with `len == 0`.
- Zero unnecessary allocations or uninitialized memory leaks on empty streams.

---

## 3. Acceptance Tests (C1–C15)

All 15 tests pass unconditionally in `crates/luna-driver/tests/stdlib_iterator_collect_acceptance_tests.rs`:

| Test ID | Name | Description | Invariant | Result |
|---|---|---|---|---|
| **C1** | `test_c1_collect_vec_basic` | Basic Vec collect from array slice iterator via deref mapping | COLLECT-1, 4 | **PASS** |
| **C2** | `test_c2_collect_vec_empty` | Empty iterator collect to Vec produces len 0 | COLLECT-5 | **PASS** |
| **C3** | `test_c3_collect_vec_filter_map_pipeline` | Adapter pipeline collect (`filter` -> `map` -> `vec`) | COLLECT-1, 4 | **PASS** |
| **C4** | `test_c4_collect_vec_droptracker` | Owned generator collect verifies all elements tracked and dropped on Vec destruction | COLLECT-2 | **PASS** |
| **C5** | `test_c5_collect_hashset_basic` | Basic HashSet collect from integer slice iterator | COLLECT-1, 3 | **PASS** |
| **C6** | `test_c6_collect_hashset_dedup` | HashSet collect deduplicates identical elements (3 items in -> 2 in set) | COLLECT-3 | **PASS** |
| **C7** | `test_c7_collect_hashset_droptracker` | DropTracker proves rejected duplicate item dropped during collect, remaining dropped with set | COLLECT-2, 3 | **PASS** |
| **C8** | `test_c8_collect_hashmap_basic` | Basic HashMap collect from key-value tuple generator | COLLECT-1 | **PASS** |
| **C9** | `test_c9_collect_hashmap_dedup_last_wins` | Duplicate key collect updates value to latest (last-wins) | COLLECT-3 | **PASS** |
| **C10** | `test_c10_collect_hashmap_droptracker_identity` | Proves HM-4/HS-3 identity: incoming key dropped, old value dropped, first key preserved | COLLECT-2, 3 | **PASS** |
| **C11** | `test_c11_collect_chained_pipeline_with_zip` | Full pipeline chaining with `zip`, `take`, and `vec` collect | COLLECT-1, 4 | **PASS** |
| **C12** | `test_c12_collect_source_vs_llib_parity` | Identical execution semantics between precompiled `.llib` and source `.ln` | Rule 6, 7 | **PASS** |
| **C13** | `test_c13_collect_vec_box_heap_elements` | Vec collect of heap-allocated `Box<i32>` values | COLLECT-2 | **PASS** |
| **C14** | `test_c14_collect_into_multiple_containers` | Single pipeline elements collected into separate Vec and HashSet | COLLECT-1 | **PASS** |
| **C15** | `test_c15_collect_hashset_droptracker_identity` | Proves HS-3 identity: original key retained, duplicate key dropped immediately | COLLECT-3 | **PASS** |

---

## 4. Rule 11 Compliance Report

Per the Luna Stdlib-Compiler Boundary protocol:
- **Zero compiler source files modified**: `git status` verifies no changes to `crates/luna-lexer`, `luna-parser`, `luna-ast`, `luna-semantic`, `luna-borrowck`, `luna-mvir`, `luna-backend`, or `luna-driver/src`.
- **Zero compiler intrinsics or opcodes**: No special-cased AST nodes, semantic branches, or MIR instructions for `collect`.
- **Pure library code**: `iter_collect_*` functions are standard library generic functions implemented strictly within Luna language syntax.

---

## 5. Artifact Parity & Full Regression

- **Precompiled Artifact**: `libs/external/alloc.llib` regenerated and synchronized with `alloc.ln`.
- **Regression Test Summary**:
  - `stdlib_iterator_collect_acceptance_tests`: 15/15 passed
  - `stdlib_iterator_terminal_consumers_acceptance_tests`: 15/15 passed
  - `stdlib_iterator_adapters_acceptance_tests`: 11/11 passed
  - `stdlib_slice_iterator_acceptance_tests`: 10/10 passed
  - `stdlib_collection_iterators_acceptance_tests`: 8/8 passed
  - `stdlib_hashmap_acceptance_tests`: 13/13 passed
  - `stdlib_hashmap_storage_acceptance_tests`: 8/8 passed
  - `stdlib_hashset_acceptance_tests`: 11/11 passed
  - `stdlib_vec_element_drop_acceptance_tests`: 10/10 passed
  - `vec_iterator_lifetime_acceptance_tests`: 8/8 passed
  - `luna-driver unittests`: 3/3 passed
  - **Total**: 112/112 passed across all active stdlib suites.
