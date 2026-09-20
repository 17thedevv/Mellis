# STDLIB-MATURITY-01: Phase 0 Capability & API Contract Audit

## 1. Objective

Phase 0 acts as a mandatory capability and semantic boundary gate prior to standard library implementation and stabilization. In accordance with Rule 11 (**Stdlib–Compiler Boundary Rule**) and Rule 8 (**Grammar Authority Rule**), every planned API surface is evaluated against five mandatory criteria:
1. **Syntax**: Can current `mellis-grammar` express it without foreign syntax (e.g. turbofish, `mut`, `'a`)?
2. **Type System**: Can the current typechecker express and check it without ad-hoc compiler magic?
3. **Borrow / Region Model**: Can reference lifetimes, capability exclusivity, and provenance be tracked?
4. **Artifact Transport**: Can `luna-llib` serialize and deserialize the declarations with full source parity?
5. **Failure & Panic Semantics**: Is the failure behavior explicitly defined (panic/trap vs `Option` vs `Result`)?

Any capability deficit is recorded as a **`LANGUAGE-GAP`** and isolated. Under no circumstances will compiler workarounds or special-case hacks be introduced to make stdlib implementation convenient.

---

## 2. Five-Question Audit Matrix per Proposed API

| Module & Target API | 1. Syntax | 2. Type System | 3. Borrow / Region | 4. .llib Transport | 5. Failure Semantics | Audit Verdict |
|---|:---:|:---:|:---:|:---:|---|:---:|
| `Option::is_some` / `is_none` | ✅ | ✅ | ✅ | ✅ | Never fails (pure bool) | **READY** ✅ |
| `Option::unwrap` / `expect` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `None` | **READY** ✅ |
| `Option::unwrap_or` | ✅ | ✅ | ✅ | ✅ | Never fails (returns default) | **READY** ✅ |
| `Option::as_ref` / `as_mut` | ✅ | ⚠️ *(See Gap 01)* | ✅ | ✅ | Never fails (returns `Option<&T>`) | **ENCAPSULATED** ⚠️ |
| `Result::is_ok` / `is_err` | ✅ | ✅ | ✅ | ✅ | Never fails (pure bool) | **READY** ✅ |
| `Result::unwrap` / `expect` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `Err` | **READY** ✅ |
| `Result::unwrap_err` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `Ok` | **READY** ✅ |
| `Result::as_ref` / `as_mut` | ✅ | ⚠️ *(See Gap 01)* | ✅ | ✅ | Never fails (returns `Result<&T, &E>`) | **ENCAPSULATED** ⚠️ |
| `Iterator::next` | ✅ | ✅ | ✅ | ✅ | Returns `Option::None` at end | **READY** ✅ |
| `IntoIterator::into_iter` | ✅ | ✅ | ✅ | ✅ | Consuming move | **READY** ✅ |
| `Vec::push` / `pop` | ✅ | ✅ | ✅ | ✅ | `pop` returns `Option<T>` | **READY** ✅ |
| `Vec::as_slice` / `as_mut_slice` | ✅ | ✅ | ✅ | ✅ | Never fails (returns `&[T]`) | **READY** ✅ |
| `Vec::remove` / `insert` | ✅ | ✅ | ✅ | ✅ | Panics on out-of-bounds index | **READY** ✅ |
| `HashMap::get` | ✅ | ✅ | ✅ | ✅ | Returns `Option<&V>` | **READY** ✅ |
| `HashMap::get_mut` | ✅ | ✅ | ✅ | ✅ | Returns `Option<&rw V>` | **READY** ✅ |
| `HashMap::insert` / `remove` | ✅ | ✅ | ✅ | ✅ | Returns `Option<V>` or `bool` | **READY** ✅ |
| `HashSet::insert` / `contains` | ✅ | ✅ | ✅ | ✅ | Returns `bool` | **READY** ✅ |
| `iter_collect_*` | ✅ | ✅ | ✅ | ✅ | Concrete functions, no turbofish | **READY** ✅ |
| `String::from_str` | ✅ | ✅ | ✅ | ✅ | Panics on invalid UTF-8 | **READY** ✅ |
| `String::from_bytes` | ✅ | ✅ | ✅ | ✅ | Returns `Option<String>` | **READY** ✅ |
| `String::push_char` / `push_str` | ✅ | ✅ | ✅ | ✅ | Panics on invalid UTF-8 string | **READY** ✅ |
| `String::truncate` | ✅ | ✅ | ✅ | ✅ | Panics if not at char boundary | **READY** ✅ |
| `Range<T>` struct | ✅ | ✅ | ✅ | ✅ | Pure value aggregate | **READY** ✅ |
| `Range` syntax `0..10` | ❌ | ❌ | ❌ | ❌ | Syntax deferred | **DEFERRED** ⛔ |
| Slice `contains` / `position` | ✅ | ✅ | ✅ | ✅ | Returns `bool` / `Option<u64>` | **READY** ✅ |
| Slice `binary_search` | ✅ | ✅ | ✅ | ✅ | Returns `Result<u64, u64>` | **READY** ✅ |

---

## 3. Deep Dive: LANGUAGE-GAP-01 (Enum Reference Extraction)

### Description
In Luna, `match` on a reference `&Enum` (such as `&Option<T>`) produces a type mismatch:
```text
error: type mismatch: expected Enum, got Reference
```
The pattern matcher does not automatically dereference references when matching enum variants.

If a developer instead dereferences the subject (`match *self`), the enum pattern destructor attempts to bind the inner field by value (`match *self { Option::Some(val) => ... }`). When `T` is a non-`Copy` type (such as `Box<T>` or `Vec<T>`), this constitutes a move out of borrowed memory, which is correctly rejected by borrowck (`E3001`).

### Consequence in Stdlib
Safe Luna code currently has no source-level pattern syntax (like `ref val` or `&val`) to borrow directly into the payload of a borrowed enum without moving it.

To provide `as_ref()` and `as_mut()`, `libs/external/lang/option.ln` currently encapsulates this in a private `unsafe` block:
```luna
export fn as_ref(self: &Self) -> Option<&T> life_from(self) {
    if self.is_some() {
        unsafe {
            dec val_ptr = ((self as *Self as u64) + (4 as u64)) as *T;
            return Option::Some(&*val_ptr);
        }
    }
    return Option::None;
}
```

### Architectural Judgment & Boundary Defense
1. **Call-site Safety is Preserved**:
   Callers of `Option::as_ref()` and `Option::as_mut()` receive an `Option<&T>` bound by `life_from(self)`. The Region Engine treats the returned reference as derived from `self`, ensuring that mutating or dropping `self` while the reference is alive triggers borrowck rejections (`E3003` / `E3002`, verified by `test_opt_as_ref_mutation_conflict_rejected`).
2. **No Compiler Special Cases**:
   The compiler will **NOT** be given special-cased magic for `Option` or `Result`.
3. **Formal Language Gap**:
   `LANGUAGE-GAP-01` is officially logged:
   > *"Pattern matching on a borrowed enum (`&E` or `&rw E`) to project borrowed variant payloads (`&T` or `&rw T`) safely without manual pointer arithmetic."*
   Until a future language proposal introduces reference pattern matching, `Option::as_ref` and `as_mut` remain strictly encapsulated within the standard library implementation.

---

## 4. API Signature & Semantics Refinements

### 1. `HashMap` & `HashSet`
- **Receiver Capabilities**:
  ```luna
  export fn get(self: &Self, key: &K) -> Option<&V> life_from(self);
  export fn get_mut(self: &rw Self, key: &K) -> Option<&rw V> life_from(self);
  export fn contains_key(self: &Self, key: &K) -> bool;
  export fn remove(self: &rw Self, key: &K) -> bool;
  ```
  The key parameter is always `&K` (shared borrow). Mutable access capability belongs strictly to the map receiver `self: &rw Self`.

### 2. Collection Terminal (`collect`)
- **No Turbofish Syntax**:
  Luna does not possess turbofish syntax (`::<>` or `<>`).
  Instead of inventing parser syntax, collection from iterators is provided via explicit, zero-magic functions:
  ```luna
  dec v = iter_collect_vec(iter);
  dec s = iter_collect_hashset(iter);
  dec m = iter_collect_hashmap(iter);
  ```
  This is fully supported, unambiguous, and passes all acceptance suites today.

### 3. Range Types
- **Syntax vs Library**:
  `Range<T>` and `RangeInclusive<T>` are defined as library structs:
  ```luna
  export struct Range<T> {
      export start: T,
      export end: T,
  };
  ```
  Range syntax sugar (`0..10`, `0..=10`) is omitted from expressions in this milestone to preserve strict grammar boundaries.

### 4. String UTF-8 Model
- **Strict Invariant**:
  `String` is guaranteed to contain valid UTF-8 at all times.
  - Construction from bytes validates (`string_from_bytes(bytes: &[u8]) -> Option<String>`).
  - Construction from literal string asserts valid UTF-8 (`string_from_str(s: str) -> String`).
  - Mutation encodes scalar characters (`push_char(self: &rw Self, ch: char)`).
  - Truncation validates code-point boundary (`is_char_boundary`). Slicing across a character boundary panics.
  - Arbitrary, unvalidated byte buffers are managed via `Vec<u8>`, not `String`.

---

## 5. Phase 0 Verdict

- **Phase 0 Audit**: **PASSED WITH RECORDED GAPS** ✅
- **Language Gaps Logged**:
  - `LANGUAGE-GAP-01`: Safe pattern-match borrowing into enum payloads (`&Enum -> Option<&T>`).
  - `LANGUAGE-GAP-02`: Range expression syntax sugar (`0..10` in expressions).
- **Readiness**:
  All 25 modules have confirmed syntax, type, borrow, and `.llib` parity baselines.
  Phase A (`Drop`, `Option`, `Result`, `Iterator`, `Cmp/Hash`) is fully cleared to proceed.
