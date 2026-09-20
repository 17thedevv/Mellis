# STDLIB-MATURITY-01: Phase 0 Capability & API Contract Audit

## 1. Objective

Phase 0 acts as a mandatory capability and semantic boundary gate prior to standard library implementation and stabilization. In accordance with Rule 11 (**Stdlib–Compiler Boundary Rule**) and Rule 8 (**Grammar Authority Rule**), every planned API surface is evaluated against five mandatory criteria:
1. **Syntax**: Can current `mellis-grammar` express it without foreign syntax (e.g. turbofish, `mut`, `'a`)?
2. **Type System**: Can the current typechecker express and check it without ad-hoc compiler magic?
3. **Borrow / Region Model**: Can reference lifetimes, capability exclusivity, and provenance be tracked?
4. **Artifact Transport**: Can `luna-llib` serialize and deserialize the declarations with full source parity?
5. **Failure & Panic Semantics**: Is the failure behavior explicitly defined (panic/trap vs `Option` vs `Result`)?

### Core Principle: Two-Way Boundary Defense
Rule 11 operates symmetrically:
- **Compiler side**: The compiler must not special-case standard library types with dedicated opcodes, magic branches, or custom borrowck exemptions.
- **Library side**: The standard library must not reverse-engineer compiler layout via offset guessing or layout punning to paper over a language semantic limitation.

---

## 2. Five-Question Audit Matrix per Proposed API

| Module & Target API | 1. Syntax | 2. Type System | 3. Borrow / Region | 4. .llib Transport | 5. Failure Semantics | Audit Verdict |
|---|:---:|:---:|:---:|:---:|---|:---:|
| `Option::is_some` / `is_none` | ✅ | ✅ | ✅ | ✅ | Never fails (pure bool) | **READY** ✅ |
| `Option::unwrap` / `expect` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `None` | **READY** ✅ |
| `Option::unwrap_or` | ✅ | ✅ | ✅ | ✅ | Never fails (returns default) | **READY** ✅ |
| `Option::as_ref` / `as_mut` | ✅ | ❌ *(See Gap 01)* | ❌ *(See Gap 01)* | ✅ | Never fails | **GAP-BLOCKED / DEFERRED** ⛔ |
| `Result::is_ok` / `is_err` | ✅ | ✅ | ✅ | ✅ | Never fails (pure bool) | **READY** ✅ |
| `Result::unwrap` / `expect` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `Err` | **READY** ✅ |
| `Result::unwrap_err` | ✅ | ✅ | ✅ | ✅ | Panics / traps on `Ok` | **READY** ✅ |
| `Result::as_ref` / `as_mut` | ✅ | ❌ *(See Gap 01)* | ❌ *(See Gap 01)* | ✅ | Never fails | **GAP-BLOCKED / DEFERRED** ⛔ |
| `Iterator::next` | ✅ | ✅ | ✅ | ✅ | Returns `Option::None` at end | **READY** ✅ |
| `IntoIterator::into_iter` | ✅ | ✅ | ✅ | ✅ | Consuming move | **READY** ✅ |
| `Vec::push` / `pop` | ✅ | ✅ | ✅ | ✅ | `pop` returns `Option<T>` | **READY** ✅ |
| `Vec::as_slice` / `as_mut_slice` | ✅ | ✅ | ✅ | ✅ | Never fails (returns `&[T]`) | **READY** ✅ |
| `Vec::remove` / `insert` | ✅ | ✅ | ✅ | ✅ | Panics on out-of-bounds index | **READY** ✅ |
| `HashMap::get` | ✅ | ✅ | ✅ | ✅ | Returns `Option<&V>` | **READY** ✅ |
| `HashMap::get_mut` | ✅ | ✅ | ✅ | ✅ | Returns `Option<&rw V>` | **READY** ✅ |
| `HashMap::insert` / `remove` | ✅ | ✅ | ✅ | ✅ | Returns `Option<V>` (previous/removed) | **READY** ✅ |
| `HashSet::insert` / `remove` | ✅ | ✅ | ✅ | ✅ | Returns `bool` (membership changed) | **READY** ✅ |
| `HashSet::contains` | ✅ | ✅ | ✅ | ✅ | Returns `bool` | **READY** ✅ |
| `iter_collect_*` | ✅ | ✅ | ✅ | ✅ | Concrete functions, no turbofish | **READY** ✅ |
| `String::from_str` | ✅ | ✅ | ✅ | ✅ | Infallible (`&str` invariant) | **READY** ✅ |
| `String::from_bytes` | ✅ | ✅ | ✅ | ✅ | Returns `Option<String>` (validates UTF-8) | **READY** ✅ |
| `String::push_char` | ✅ | ✅ | ✅ | ✅ | Infallible (`char` invariant) | **READY** ✅ |
| `String::push_str` | ✅ | ✅ | ✅ | ✅ | Infallible (`&str` invariant) | **READY** ✅ |
| `String::truncate` | ✅ | ✅ | ✅ | ✅ | Panics if not at char boundary | **READY** ✅ |
| `Range<T>` struct | ✅ | ✅ | ✅ | ✅ | Pure value aggregate | **READY** ✅ |
| `Range` syntax `0..10` | ❌ | ❌ | ❌ | ❌ | Syntax deferred | **DEFERRED** ⛔ |
| Slice `contains` / `position` | ✅ | ✅ | ✅ | ✅ | Returns `bool` / `Option<u64>` | **READY** ✅ |
| Slice `binary_search` | ✅ | ✅ | ✅ | ✅ | Returns `Result<u64, u64>` | **READY** ✅ |

---

## 3. Deep Dive: LANGUAGE-GAP-01 (Borrowed Enum Payload Projection)

### Description & Root Cause
In Luna, `match` on a reference `&Enum` (such as `&Option<T>`) produces a type mismatch:
```text
error: type mismatch: expected Enum, got Reference
```
The pattern matcher expects the pattern subject to match the nominal enum type, and does not automatically dereference references when matching enum variants.

If a developer instead dereferences the subject (`match *self`), the enum pattern destructor attempts to bind the inner field by value (`match *self { Option::Some(val) => ... }`). When `T` is a non-`Copy` type (such as `Box<T>` or `Vec<T>`), this constitutes a move out of borrowed memory, which is correctly rejected by borrowck (`E3001`).

Safe Luna code currently lacks source-level pattern syntax (such as `ref val` or `&val`) or match-on-reference semantics to project borrowed references into enum payloads without moving them.

### Rejection of Manual Layout Offset Punning
In prior prototype code (`libs/external/lang/option.ln`), `as_ref` and `as_mut` bypassed the type system using raw pointer arithmetic:
```luna
dec val_ptr = ((self as *Self as u64) + (4 as u64)) as *T;
```
**This layout assumption is fundamentally unsound**:
- Hardcoding `+ 4` assumes a 4-byte discriminant followed immediately by payload without padding.
- For any type `T` with alignment > 4 (such as `u64`, pointers on 64-bit systems, or 8-byte aligned structs), compiler ABI requirements place padding between the 4-byte discriminant and the payload.
  - Example: `Option<u64>` layout is `[tag: 4 bytes][padding: 4 bytes][payload: 8 bytes]`.
  - Adding `+ 4` computes a pointer directly into the uninitialized padding bytes!
- Furthermore, pointer-to-u64 arithmetic is non-portable and creates tight coupling between stdlib source and internal compiler struct/enum lowering conventions.

### Architectural Separation: Provenance vs Layout
Existing test `test_opt_as_ref_mutation_conflict_rejected` validates that:
$$\text{Returned reference lifetime correctness: } \Gamma \vdash life\_from(self) \text{ properly constrains call-site loans.}$$
However, lifetime constraint propagation at the call-site does NOT imply payload address correctness:
$$\text{Payload address correctness } \neq \text{ Returned reference lifetime correctness}$$

### Verdict & Strategic Action for LANGUAGE-GAP-01
- **Status**: **CONFIRMED LANGUAGE GAP ⚠️**
- **Manual layout arithmetic**: **STRICTLY REJECTED AS FROZEN STDLIB CODE ❌**
- **Action**:
  - The four borrow-view APIs:
    - `Option::as_ref`
    - `Option::as_mut`
    - `Result::as_ref`
    - `Result::as_mut`
    are **`GAP-BLOCKED / DEFERRED`** from Phase A v1.
  - All value-based, query, unwrapping, and combinator APIs of `Option` and `Result` are **100% sound, safe, and fully cleared**.
  - A future dedicated language milestone will design generic borrowed enum pattern matching:
    ```luna
    match self {
        Option::Some(ref value) -> ...
    }
    ```
    without adding any Option-specific compiler opcodes or layout hacks.

---

## 4. API Signature & Semantics Refinements

### 1. `HashMap` & `HashSet` Canonical Contracts
- **`HashMap<K, V>`**:
  - Receiver capability: Querying a value borrow requires `&rw Self` for mutable access, but key is always `&K` (shared borrow).
  - Explicit insertion & removal:
    ```luna
    export fn get(self: &Self, key: &K) -> Option<&V> life_from(self);
    export fn get_mut(self: &rw Self, key: &K) -> Option<&rw V> life_from(self);
    export fn insert(self: &rw Self, key: K, val: V) -> Option<V>; // returns previous value if overwritten
    export fn remove(self: &rw Self, key: &K) -> Option<V>;       // returns removed value if present
    export fn contains_key(self: &Self, key: &K) -> bool;
    export fn clear(self: &rw Self);
    ```
- **`HashSet<T>`**:
  ```luna
  export fn insert(self: &rw Self, val: T) -> bool; // returns true if newly inserted
  export fn remove(self: &rw Self, val: &T) -> bool; // returns true if removed
  export fn contains(self: &Self, val: &T) -> bool;
  export fn clear(self: &rw Self);
  ```

### 2. Collection Terminal (`collect`)
- **Zero-Magic Concrete Functions**:
  Luna does not possess turbofish syntax (`::<>` or `<>`). Iterators are collected using explicit, type-checked functions:
  ```luna
  dec v = iter_collect_vec(iter);
  dec s = iter_collect_hashset(iter);
  dec m = iter_collect_hashmap(iter);
  ```

### 3. Range Types
- **Library Abstraction**:
  `Range<T>` and `RangeInclusive<T>` are defined as library structs:
  ```luna
  export struct Range<T> {
      export start: T,
      export end: T,
  };
  ```
  Range syntax sugar (`0..10`, `0..=10`) is deferred; no parser features will be snuck into this stdlib milestone.

### 4. String UTF-8 Model & Invariants
- **Infallible Operations on Valid Types**:
  - `&str` and `char` in Luna guarantee valid UTF-8 by type invariant.
  - `string_from_str(s: str) -> String`: **Infallible** construction from valid `str`.
  - `push_str(self: &rw Self, s: str)`: **Infallible** append of valid UTF-8.
  - `push_char(self: &rw Self, ch: char)`: **Infallible** append of valid Unicode scalar.
- **Fallible Operations on Arbitrary Bytes**:
  - `string_from_bytes(bytes: &[u8]) -> Option<String>`: Validates UTF-8, returns `None` on invalid bytes.
- **Boundary Traps**:
  - `truncate(self: &rw Self, new_len: u64)`: Panics/traps if `new_len` is not at a UTF-8 character boundary (`!is_char_boundary`).
  - Arbitrary unvalidated byte buffers belong in `Vec<u8>`, not `String`.

---

## 5. Phase 0 Verdict

- **Phase 0 Audit**: **PASSED WITH ISOLATED LANGUAGE GAPS ✅**
- **Language Gaps Logged**:
  - `LANGUAGE-GAP-01`: Safe pattern-match borrowing into enum payloads (`&Enum -> Option<&T>`).
    - *Policy*: No layout offset punning (`+ 4`). Borrow-view APIs `Option::as_ref`, `Option::as_mut`, `Result::as_ref`, `Result::as_mut` are **GAP-BLOCKED / DEFERRED** from Phase A v1.
  - `LANGUAGE-GAP-02`: Range expression syntax sugar (`0..10` in expressions).
    - *Policy*: Deferred. `Range<T>` library aggregate remains allowed.
- **Phase A Clearance**:
  **Phase A is CLEARED ✅** for all value-based methods of `Drop`, `Option`, `Result`, `Iterator`, `IntoIterator`, and `Clone`/`Cmp`/`Hash` foundations.
