# Freeze Audit: Stdlib-05.4C HashMap & HashSet Iterators

## 1. Feature Description
This feature implements `IntoIterator` and `Iterator` for `HashMap<K, V>` and `HashSet<T>` in the Luna standard library (`<alloc>`), covering both owned (`map.into_iter()`) and borrowed (`map.iter()`) iteration paths, as well as their `for ... in` abstractions.

## 2. Tested Semantic Invariants

### Ownership and Drop Parity
- **ITER-HMHS-OWN-1**: Every occupied key/value reached by the owning iterator is destroyed exactly once across normal completion and early break.
- **ITER-HMHS-OWN-2**: Iterator destruction releases remaining occupied storage exactly once.

### Borrowing and Lifetimes
- **ITER-HMHS-BORROW-1 (Negative)**: The borrow checker rejects scenarios where borrowed items (`&K`, `&V`) outlive the collection scope (throws `E3005: LocalBorrowEscape`).
- **ITER-HMHS-BORROW-2 (Positive)**: The borrow checker correctly releases the borrow lock when the iterator scope ends, allowing subsequent collection mutation.

## 3. Artifact Compatibility
- **Format/Parser Compatibility**: YES
- **Legacy Semantic Modality**: NOT guaranteed (due to metadata parity changes)
- **Rebuild Required**: YES

## 4. Verification Evidence
All validation was executed strictly using `.ln` CLI fixtures (Layer B) against both source `.ln` and precompiled `.llib` artifacts (Layer C), proving complete execution parity without relying on internal compiler test harnesses.

- `tests/luna/stdlib/iterators/hashmap_into_iter_owned.ln`
- `tests/luna/stdlib/iterators/hashmap_into_iter_borrowed.ln`
- `tests/luna/stdlib/iterators/hashmap_for_in_break.ln`
- `tests/luna/stdlib/iterators/hashmap_drop_tracker.ln`
- `tests/luna/stdlib/iterators/hashmap_borrow_reject.ln`
- `tests/luna/stdlib/iterators/hashmap_borrow_accept.ln`
- `tests/luna/stdlib/iterators/hashset_into_iter_owned.ln`
- `tests/luna/stdlib/iterators/hashset_into_iter_borrowed.ln`
- `tests/luna/stdlib/iterators/hashset_for_in_break.ln`
- `tests/luna/stdlib/iterators/hashset_drop_tracker.ln`
- `tests/luna/stdlib/iterators/hashset_borrow_reject.ln`
- `tests/luna/stdlib/iterators/hashset_borrow_accept.ln`
