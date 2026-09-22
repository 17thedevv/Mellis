# STDLIB-CORE-DEPTH-AUDIT-01 — Core Standard Library Surface & Depth Audit

**Document**: `docs/spec/stdlib-core-depth-audit.md`  
**Milestone**: `Stage 7 — Phase 3: Core Stdlib Surface & Depth Audit`  
**Status**: `CANONICAL AUDIT COMPLETE ✅`  
**Compliance**: Verified against `mellis-grammar`, `luna-semantic-compliance`, `luna-stdlib-compiler-boundary`, `BOX-FROZEN`, and `STD-ARCH-01`.

---

## 1. Executive Summary & Strategic Rationale

Following the closure of **Phase 0** (Capability Audit), **Phase 1** (Practical I/O & Core Types), and **Phase 2** (Text, Parsing & Formatting), this audit addresses the foundational depth of Luna's standard library.

A direct comparison of Luna's stdlib against the Rust standard library (`std`) and the C++ Standard Library (`ISO/IEC 14882`) reveals that while Luna possesses a sound systems foundation (provenance, region-based borrowing, monomorphization, `.llib` artifact identity, and runtime ABI v1), its **general-purpose core library surface** is thin.

Jumping directly to operating system abstractions (Filesystem, Path, Process Environment) without a robust core library forces OS APIs to invent ad-hoc conventions. A systems language standard library requires a coherent, shared vocabulary:
1. Deep collection and slice manipulation APIs.
2. An extensive iterator and algorithm pipeline.
3. Monadic combinators for error flow (`Option`, `Result`).
4. Unified conversion and default protocols.
5. Complete numeric, floating-point, and memory utilities.
6. A principled evaluation of shared ownership and concurrency safety.

### Strategic Checklist Principle
> **The Rust and C++ Standard Libraries serve as an exhaustive checklist to discover forgotten domains and shallow abstractions. They are NOT templates to be copied blindly into Luna.**
>
> Every capability in Luna must strictly respect:
> - **Explicit Lifetime Relations** (`life_from`, `where outlives`).
> - **Strict Mutability Semantics** (`&rw` exclusive borrowing).
> - **Region Validity vs Borrow Legality**: Lifetime validity (`life_from`) does NOT automatically prove non-overlapping capability exclusivity.
> - **Rule 10 (`BOX-FROZEN`)**: Zero compiler magic or distinguished opcodes for library containers.
> - **Rule 11 (`luna-stdlib-compiler-boundary`)**: Stdlib is a pure language consumer.

---

## 2. Comprehensive Inventory of Current Sysroot Baseline

The table below documents the exact physical baseline of Luna's **26 canonical sysroot providers** (`libs/external/`):

| Layer | Provider | Primary Types / Functions | Current Depth & State |
| :--- | :--- | :--- | :--- |
| **`lang/`** | `drop` | `trait Drop { fn drop(&rw self); }` | Frozen language contract |
| | `iterator` | `trait Iterator<Item> { fn next(&rw self) -> Option<Item>; }` | Minimal trait definition |
| | `into_iterator` | `trait IntoIterator<Item, I: Iterator<Item>> { fn into_iter(self) -> I; }` | Minimal conversion contract |
| | `option` | `enum Option<T> { Some(T), None }` | Basic methods (`is_some`, `is_none`, `unwrap`, `expect`, `unwrap_or`, `map`, `and_then`) |
| **`core/`** | `clone` | `trait Clone { fn clone(&self) -> Self; }` | Implemented for scalar primitives (excluding `f32`/`f64`) |
| | `copy` | `trait Copy {}` | Marker lang item |
| | `cmp` | `trait Eq`, `trait Ord` | Implemented for integer primitives; `cmp` returns `i32` (`-1`, `0`, `1`) |
| | `hash` | `trait Hash { fn hash(&self) -> u64; }` | Implemented for scalar primitives and `String` |
| | `slice` | `SliceIter<T>`, `SliceIterMut<T>`, `contains`, `position`, `binary_search`, `reverse`, `sort` | Basic algorithms; sort is $O(n^2)$ insertion sort |
| | `iter_adapters` | `Map`, `Filter`, `Enumerate`, `Take`, `Skip`, `Zip`, `Range`, `RangeInclusive` | Free functions (`iter_map`, etc.); `Range` hardcoded to `i32` |
| | `iter_consumers` | `iter_fold`, `iter_count`, `iter_for_each`, `iter_any`, `iter_all`, `iter_find` | Free functions |
| | `mem` | `size_of`, `align_of`, `copy`, `set`, `zero`, `slice_from_raw_parts`, `slice_from_raw_parts_mut` | Low-level byte and slice primitives |
| | `ptr` | `read`, `write`, `drop_in_place`, `add`, `add_mut`, `offset`, `offset_mut`, `diff`, `copy` | Element-strided pointer arithmetic |
| | `result` | `enum Result<T, E> { Ok(T), Err(E) }` | Basic methods (`is_ok`, `is_err`, `unwrap`, `expect`, `map`, `map_err`, `and_then`, `ok_or`) |
| | `try` | `trait Try`, `trait FromResidual` | Desugaring contract for `?` |
| | `panic` | `__luna_panic_default()` | Direct runtime trap |
| **`alloc/`** | `box` | `struct Box<T>` | Unique heap pointer (`as_ref`, `as_mut`, `into_inner`, `Drop`) |
| | `global` | `__luna_alloc`, `__luna_dealloc`, `__luna_realloc` | Direct allocator bindings |
| | `vec` | `struct Vec<T>` | Growable array (`push`, `pop`, `insert`, `remove`, `get`, `get_mut`, `truncate`, `as_slice`) |
| | `string` | `struct String` | UTF-8 byte vector (`find`, `split`, `lines`, `trim`, `parse_*`, `string_from_*`) |
| | `raw_table` | `struct RawTable<K, V>` | Open-addressing quadratic-probing hash table |
| | `hashmap` | `struct HashMap<K, V>` | Key-value map (`insert`, `get`, `get_mut`, `remove`, `contains_key`, `keys`, `values`) |
| | `hashset` | `struct HashSet<T>` | Key-set wrapper around `RawTable<T, ()>` |
| | `iter_collect` | `trait FromIterator`, `iter_collect_vec` | Collect iterator into `Vec` |
| **`io/`** | `io` | `print`, `println`, `eprintln`, `read_line`, `read_to_string`, `write_string` | Whole-file and console I/O |
<!-- process/ provider retired in C-GAP-11; CLI arguments delivered exclusively via fn main(args: [str]) -->

*Note: Canonical design does not assume every new concept equals a new provider file. Providers group cohesive logical modules under sysroot conventions.*

---

## 3. The 13 Core Domain Audits

### Domain 1: Collections Surface & Depth

#### Reference Baseline
- **Rust**: `Vec` (over 80 methods: `retain`, `dedup`, `resize`, `drain`, `extend`, `swap_remove`, `split_off`), `HashMap` (`Entry` API: `or_insert`, `or_default`, `and_modify`), `HashSet` (set algebra), `VecDeque`, `BTreeMap`, `BinaryHeap`.
- **C++**: `std::vector`, `std::unordered_map`, `std::unordered_set`, `std::deque`, `std::map`, `std::priority_queue`.

#### Current Luna Baseline
- `Vec<T>` provides `push`, `pop`, `insert`, `remove`, `get`, `get_mut`, `clear`, `truncate`, `last`, `as_slice`.
- `HashMap<K, V>` provides `insert`, `get`, `get_mut`, `remove`, `contains_key`, `clear`, `iter`, `keys`, `values`.
- `HashSet<T>` provides `insert`, `contains`, `remove`, `clear`, `iter`.

#### Gap Analysis & Classifications
1. **`Vec::swap_remove`** (`PROVEN-CORE-SURFACE`): $O(1)$ removal by swapping with the last element. Moves the removed element soundly without arbitrary bit-copying.
2. **`Vec::retain`** (`PROVEN-CORE-SURFACE`): In-place filtering preserving order.
3. **`Vec::dedup`** (`PROVEN-CORE-SURFACE`): Removes consecutive duplicates; requires `T: Eq`.
4. **`Vec::extend`** (`PROVEN-CORE-SURFACE`): Appends elements in bulk from an iterator or slice.
5. **`Vec::resize`** (`PROVEN-CORE-SURFACE`): Must enforce explicit type contracts: `resize(new_len, value)` requires `T: Clone` (or `resize_with(new_len, generator)`). Does not bit-copy arbitrary `T`.
6. **`Vec` Trait Implementations** (`PROVEN-CORE-SURFACE`): `impl<T: Clone> Clone for Vec<T>` and `impl<T: Eq> Eq for Vec<T>`.
7. **`HashMap` Entry API Feasibility** (`FEASIBILITY-PROBE-REQUIRED`):
   - A canonical `Entry` API requires an intermediate object carrying an exclusive mutable borrow into map storage:
     $$\text{map.entry(key)} \longrightarrow \text{OccupiedEntry} \mid \text{VacantEntry} \longrightarrow \text{entry.or_insert(v)} \longrightarrow \text{\&rw V tied to map}$$
   - **Semantic Risk**: Pattern matching over an `Entry` enum while preserving borrow exclusivity touches `LANGUAGE-GAP-01` (Borrowed enum payload projection) and `LANGUAGE-GAP-04`.
   - **Protocol**: Probe in Phase 4A. If borrowck cannot preserve exclusive projection, classify as `LANGUAGE-BLOCKED` and provide sound v1 alternatives (`get_or_insert(key, default)`, `insert_if_absent(key, value)`) rather than faking an unsound Entry struct.
8. **`HashMap` return semantics** (`PROVEN-CORE-SURFACE`): `insert` and `remove` currently return `bool` instead of `Option<V>` (returning the previous or removed value).
9. **`HashSet` set algebra** (`DESIGN-REQUIRED`):
   - APIs like `union`, `intersection`, `difference` have two distinct designs:
     - *Alternative A*: Lazy iterator yielding `&T` (constrained by `LANGUAGE-GAP-04`).
     - *Alternative B*: Allocating a new `HashSet<T>` (requires `T: Clone` + allocation).
   - Must decide Luna's standard design before implementation.
10. **Secondary Containers (`VecDeque`, `BTreeMap`, `BinaryHeap`)** (`DEFERRED-BEYOND-V1`): Evaluated as package-level candidates; not blocking for v1 core stdlib.

---

### Domain 2: Iterator Ecosystem & Adapters

#### Reference Baseline
- **Rust**: `Iterator` combinators: `map`, `filter`, `flat_map`, `zip`, `enumerate`, `peekable`, `filter_map`, `take`, `skip`, `chain`, `fold`, `all`, `any`, `find`, `position`, `collect`.
- **C++**: Ranges library (`std::ranges::views`) and STL algorithms.

#### Current Luna Baseline & Architectural Reality
- `Iterator<Item>` and `IntoIterator` exist as core traits.
- Adapters exist as separate structs in `core/iter_adapters.ln`: `Map`, `Filter`, `Enumerate`, `Take`, `Skip`, `Zip`, `Range`, `RangeInclusive`.
- Consumers exist in `core/iter_consumers.ln`: `iter_fold`, `iter_count`, `iter_for_each`, `iter_any`, `iter_all`, `iter_find`.
- **No Trait Default Method Lowering in v1**: In Luna, method lookup queries `Decl::Impl` directly. Traits cannot synthesize default method bodies across arbitrary implementors in v1. Thus, Luna adapters are invoked via free functions (`iter_map(it, f)`) or inherent struct methods, not fluent method chaining on `Iterator`.

#### Gap Analysis & Classifications
1. **Proven Iteration Consumers** (`PROVEN-CORE-SURFACE`):
   - `iter_position`: Returns the index of the first matching element.
   - `iter_nth`: Consumes $n$ elements and returns the $(n+1)$-th.
   - `iter_last`: Consumes iterator to return the final element.
   - `iter_sum`: Additive reduction.
2. **Simple Adapters** (`PROVEN-CORE-SURFACE`):
   - `Chain<A, B>`: Concatenates two iterators sequentially.
3. **Carrier Lifetime Adapters** (`FEASIBILITY-PROBE-REQUIRED`):
   - Adapters like `Peekable<I>` retain an `Item` internally. When `Item = &T`, the adapter becomes a carrier of borrowed provenance.
   - **Protocol**: Phase 4A must probe against Region authority with adversarial cases: peek reference, move `Peekable`, drop source, loop-carried `Peekable`, and nested adapters. Do NOT rely on borrow heuristics.
4. **Canonical Range Coverage** (`PROVEN-CORE-SURFACE`):
   - Luna already has generic `Range<T>` and `RangeInclusive<T>`.
   - Do NOT invent `RangeU64`. The gap is simply expanding iterator implementations for canonical integer types (`u64`, `usize`, etc.).
   - `LANGUAGE-GAP-02` remains separate for syntax sugar (`0..10`).
5. **Lending Iterators** (`LANGUAGE-BLOCKED`): Tracked under `LANGUAGE-GAP-04`.

---

### Domain 3: Slice Surface & Memory Layout

#### Reference Baseline
- **Rust**: `[T]` sub-slicing (`split_at`, `split_at_mut`), chunking (`chunks`, `windows`), bulk copying (`copy_from_slice`), element queries.
- **C++**: `std::span` with contiguous memory mechanics.

#### Current Luna Baseline
- `core/slice.ln` contains fat pointer slices `&[T]` and `&rw [T]`.
- Implemented: `slice_iter`, `slice_iter_mut`, `contains`, `position`, `binary_search`, `reverse`, `sort`.

#### Gap Analysis & Classifications
1. **`split_at_mut` Semantic Probe** (`FEASIBILITY-PROBE-REQUIRED`):
   - `split_at_mut(slice, n) -> (&rw [T], &rw [T])` returns two mutable borrows into the same root allocation.
   - **Crucial Distinction**: `Region validity != Borrow legality`. Explicit `life_from` only establishes validity; it does NOT prove capability exclusivity or non-overlapping ranges to the borrow checker.
   - **Protocol**: Phase 4A must probe whether Luna's semantic rules can prove disjointness without unsafe aliasing violations. If no semantic mechanism exists, classify as `LANGUAGE-BLOCKED`.
2. **Safe Sub-slicing & Element Access** (`PROVEN-CORE-SURFACE`):
   - `split_at(s: &[T], mid: u64) -> (&[T], &[T]) life_from(s)` (shared borrows are trivially sound).
   - `s.get(index) -> Option<&T>` and `s.get_mut(index) -> Option<&rw T>`.
3. **Bulk Memory Operations with Strict Type Contracts** (`PROVEN-CORE-SURFACE`):
   - `copy_from_slice<T: Copy>(dst: &rw [T], src: &[T])`: Must strictly require `T: Copy`. Specify `memmove` (overlapping allowed) vs `memcpy` (guaranteed disjoint).
   - `fill(s: &rw [T], val: T)`: Requires `T: Clone` (or `T: Copy`).
4. **Windowing & Chunking** (`PROVEN-CORE-SURFACE`):
   - `chunks(s: &[T], size: u64)` and `windows(s: &[T], size: u64)` iterators.
5. **Prefix & Suffix Inspection** (`PROVEN-CORE-SURFACE`):
   - `starts_with`, `ends_with`, `strip_prefix`.

---

### Domain 4: Core Algorithms

#### Reference Baseline
- **Rust**: `sort` (pattern-defeating quicksort), `sort_unstable`, `binary_search`, `partition_point`, `dedup`.
- **C++**: `<algorithm>` (`std::sort`, `std::partition`, `std::rotate`, `std::min_element`).

#### Current Luna Baseline
- `slice::sort` uses $O(n^2)$ insertion sort.
- `slice::binary_search` is basic binary search returning `Result<u64, u64>`.
- `slice::reverse` uses two-pointer pointer swapping.

#### Gap Analysis & Classifications
1. **Sorting Complexity & Contract** (`PROVEN-CORE-SURFACE`):
   - *Correction*: Plain dual-pivot quicksort has worst-case $O(n^2)$. If guaranteed $O(n \log n)$ is required, introsort (quicksort switching to heapsort on recursion limit) must be specified.
   - Must specify: stable vs unstable (`sort` vs `sort_unstable`), comparator contract, behavior under inconsistent comparators, and sound move/swap semantics without raw byte-copying of non-Copy types.
2. **Comparator-based Sorting** (`PROVEN-CORE-SURFACE`):
   `sort_by(s, compare_fn)` allowing custom ordering without `Ord`.
3. **In-place Partitioning** (`PROVEN-CORE-SURFACE`):
   `partition<T>(s: &rw [T], pred: fn(&T) -> bool) -> u64`.
4. **Min/Max Element Search** (`PROVEN-CORE-SURFACE`):
   `min_element(s: &[T]) -> Option<&T>` and `max_element(s: &[T]) -> Option<&T>`.

---

### Domain 5: Option & Result Combinators

#### Reference Baseline
- **Rust**: Monadic composition: `and_then`, `or_else`, `unwrap_or_else`, `filter`, `transpose`, `flatten`, `map_err`.
- **C++**: `std::optional` (monadic operations in C++23) and `std::expected`.

#### Current Luna Baseline
- `Result<T, E>` in `core/result.ln`: `is_ok`, `is_err`, `ok`, `err`, `unwrap`, `expect`, `unwrap_err`, `unwrap_or`, `map`, `map_err`, `and_then`, `ok_or`.
- `Option<T>` in `lang/option.ln`: `is_some`, `is_none`, `ok`, `unwrap`, `expect`, `unwrap_or`, `map`, `and_then`.

#### Gap Analysis & Classifications
1. **Deferred / Lazy Defaults** (`PROVEN-CORE-SURFACE`):
   `unwrap_or_else(self, f: fn() -> T) -> T` for both `Option` and `Result`.
2. **Alternative Recovery** (`PROVEN-CORE-SURFACE`):
   - `Option::or(self, optb: Option<T>) -> Option<T>`
   - `Option::or_else(self, f: fn() -> Option<T>) -> Option<T>`
   - `Result::or_else<F>(self, f: fn(E) -> Result<T, F>) -> Result<T, F>`
3. **Transposition & Flattening** (`PROVEN-CORE-SURFACE`):
   - `Option::flatten(Option<Option<T>>) -> Option<T>`
   - `Option::transpose(Option<Result<T, E>>) -> Result<Option<T>, E>`
   - `Result::transpose(Result<Option<T>, E>) -> Option<Result<T, E>>`
4. **Borrowed Payload Projections (`as_ref`, `as_mut`)** (`LANGUAGE-BLOCKED`):
   Tracked under `LANGUAGE-GAP-01`.

---

### Domain 6: Unified Conversion Protocols

#### Reference Baseline
- **Rust**: `From<T>`, `Into<T>`, `TryFrom<T>`, `TryInto<T>`, `AsRef<T>`, `Borrow<T>`, `FromStr`, `ToString`.
- **C++**: Conversion constructors, conversion operators, type traits.

#### Current Luna Baseline
- Disparate named functions: `string_from_i64`, `parse_i64`, `as_bytes`.

#### Conceptual Design Required (Phase 4A)
- Do NOT blindly import Rust's vocabulary. A mini-design must establish:
  1. What does conversion mean in Luna? (Total ownership conversion vs fallible conversion vs borrowed view).
  2. Does Luna want `From`/`Into`, or a unified `Convert`/`TryConvert` model?
  3. Avoid premature `ToString`: Phase 2 established primitive formatting and Phase 4 will design `Display`/formatting. Do NOT create conflicting string conversion abstractions.

---

### Domain 7: Default & Value Protocols

#### Reference Baseline
- **Rust**: `trait Default { fn default() -> Self; }`.
- **C++**: Default constructors (`T()`).

#### Conceptual Design Required (Phase 4A)
- Does Luna need a canonical default-construction protocol?
- Evaluate whether `trait Default` should be introduced for container initialization, zeroed states, and generic algorithms.

---

### Domain 8: Numeric Utilities

#### Reference Baseline
- **Rust**: `checked_*`, `saturating_*`, `wrapping_*`, `abs`, `signum`, `min`, `max`, `clamp`, `pow`, `count_ones`, `leading_zeros`.
- **C++**: `<numeric>`, `<cmath>`, `<bit>`.

#### Current Luna Baseline & Architectural Discovery
- Semantic typechecker ALREADY supports inherent method resolution on primitive types (`ImplSelfTypeKey::Primitive(*b)`). Methods can be declared directly on `i32`, `u64`, etc.!

#### Gap Analysis & Classifications
1. **Basic Math Utilities** (`PROVEN-CORE-SURFACE`):
   `abs()`, `signum()`, `min(other)`, `max(other)`, `clamp(min, max)`, `pow(exp)`.
2. **Bitwise Inspection** (`PROVEN-CORE-SURFACE`):
   `count_ones()`, `count_zeros()`, `leading_zeros()`, `trailing_zeros()`, `rotate_left(n)`, `rotate_right(n)`.
3. **Checked Arithmetic** (`PROVEN-CORE-SURFACE`):
   `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `checked_rem` returning `Option<T>`.
4. **Saturating Arithmetic** (`PROVEN-CORE-SURFACE`):
   `saturating_add`, `saturating_sub`, `saturating_mul`.
5. **Wrapping Arithmetic** (`PROVEN-CORE-SURFACE`):
   `wrapping_add`, `wrapping_sub`, `wrapping_mul`.

---

### Domain 9: Floating-Point Math (`f32`, `f64`)

#### Current Luna Baseline & Compiler Defect
- Semantic types `BuiltinType::F32` and `BuiltinType::F64` exist.
- Lowering to LLVM `f32_type()` and `f64_type()` exists in `luna-backend`.
- **Compiler Defect (`C-GAP-09`)**: `Instruction::Add`, `Sub`, `Mul`, `Div` in `luna-backend/src/llvm_codegen.rs` unconditionally call `build_int_add`, `build_int_sub`, `build_int_mul`, panicking on float operands.

#### Gap Analysis & Classifications
1. **`C-GAP-09`: Float Backend Semantic Completion** (`COMPILER-GAP`):
   Full audit and repair required in Phase 4A:
   - Float literal lowering.
   - Arithmetic: `add`, `sub`, `mul`, `div`, `rem`, negation.
   - Floating-point comparisons (`build_float_compare`).
   - Casts: `f32` $\leftrightarrow$ `f64`, integer $\leftrightarrow$ float.
   - Function arguments and return values.
   - NaN and infinity handling.
   - Comptime evaluation parity and constant folding.
2. **Float Math Strategy (`CORE-GAP-18`)** (`DESIGN-REQUIRED`):
   - Do NOT rush to push every math function into the Luna Runtime ABI.
   - Audit options: direct FFI to system `libm`, runtime portability shim, LLVM math intrinsics, or pure Luna where reasonable.
   - Begin `core/math` only after primitive float semantics are verified green.

---

### Domain 10: Memory Utilities & Low-Level Storage

#### Current Luna Baseline
- `core/mem.ln`: `size_of`, `align_of`, `copy`, `set`, `zero`, `slice_from_raw_parts`.
- `core/ptr.ln`: `read`, `write`, `drop_in_place`, `add`, `offset`, `diff`, `copy`.

#### Gap Analysis & Classifications
1. **`mem::swap`** (`PROVEN-CORE-SURFACE`):
   Swaps values of two mutable references without invalidating either:
   ```luna
   export fn swap<T>(a: &rw T, b: &rw T) {
       unsafe {
           dec tmp = ptr::read<T>(a as *T);
           ptr::write<T>(a, ptr::read<T>(b as *T));
           ptr::write<T>(b, tmp);
       }
   }
   ```
2. **`mem::replace`** (`PROVEN-CORE-SURFACE`):
   Replaces value at destination and returns the old value.
3. **`mem::take`** (`PROVEN-CORE-SURFACE`):
   Replaces value at destination with default value and returns the old value.
4. **Uninitialized Storage Abstraction** (`DESIGN-REQUIRED`):
   Evaluate safe container for uninitialized memory slots to prevent UB in collections.

---

### Domain 11: Atomics & Concurrency Primitives

#### Current Luna Baseline
- Runtime C implementation (`runtime/src/platform/windows/sync.c`, `linux/sync.c`) contains mutex, condvar, and thread primitives.
- Compiler / MVIR / Backend has NO atomic instructions or intrinsics.
- Stdlib has NO sync wrapper types.

#### Gap Disposition
- **`AUDITED / DEFERRED TO STAGE 7 — PHASE 7`**:
  Atomics (`AtomicBool`, `AtomicUsize`) and concurrency wrappers (`Mutex<T>`, `Condvar`) are formally deferred to **Phase 7 (Time / Thread / Sync / Platform Utilities)** to preserve focus on single-threaded core data structures in Phase 4.

---

### Domain 12: Formatting & Error Protocols

#### Current Luna Baseline
- Primitive formatting functions exist (`string_from_i64`, `string_from_bool`, etc.).
- No common formatting or error traits exist.

#### Conceptual Design Required (Phase 4A)
- Does formatting need separate human-facing (`Display`) and debug-facing (`Debug`) protocols?
- What is an `Error` semantically in Luna? (Does it expose a human message, error code, or cause chain? How does it interact with `dyn Trait`?).
- Design in Phase 4A before creating empty or overlapping trait abstractions.

---

### Domain 13: Ownership Extensions & Shared Ownership

#### Current Luna Baseline & Rule 10 (`BOX-FROZEN`)
- Unique ownership is primary: `Box<T>`, `Vec<T>`, `String`.
- Any shared ownership abstraction must be expressible strictly through existing generic ownership, move, drop, borrow, and memory primitives.

#### Concurrency & Soundness Reality Check
- While pure-Luna reference counting (`Rc<T>`) is structurally expressible via `RcBox<T> { strong: u64, value: T }`, the claim of "100% sound" is premature:
  - **The Concurrency Problem**: If Luna values can cross threads (via runtime threading) without a static `Send`/`Sync`-like restriction, a non-atomic `Rc` will suffer data races.
  - **Protocol**: Phase 4A must evaluate whether Luna can statically restrict `Rc` to a single thread. If not, `Rc` must be deferred until thread safety protocols are designed.

---

## 4. Synthesis: The 20 Findings Classified

Rather than treating all 20 findings as an undifferentiated implementation backlog, they are formally partitioned into four distinct categories:

### Group A: Proven Core Surface Gaps (Ready for Phase 4B)
1. **`CORE-GAP-01`**: `Vec` depth (`swap_remove`, `retain`, `dedup`, `extend`, `resize` with `Clone` contract).
2. **`CORE-GAP-02`**: `Vec` trait impls (`impl Clone`, `impl Eq`).
3. **`CORE-GAP-06`**: Iterator consumers (`position`, `nth`, `last`, `sum`).
4. **`CORE-GAP-07`**: `Range<T>` canonical integer coverage.
5. **`CORE-GAP-09`**: Bulk slice copying (`copy_from_slice<T: Copy>`, `fill`).
6. **`CORE-GAP-10`**: Generic sorting with bounded complexity and move soundness.
7. **`CORE-GAP-11`**: `Option` & `Result` combinators (`unwrap_or_else`, `or_else`, `flatten`, `transpose`).
8. **`CORE-GAP-14`**: Primitive integer utilities (`abs`, `min`, `max`, `clamp`, `checked_*`, `saturating_*`, `wrapping_*`).
9. **`CORE-GAP-15`**: Memory primitives (`mem::swap`, `mem::replace`, `mem::take`).

### Group B: Semantic Feasibility Probes Required (Phase 4A)
10. **`CORE-GAP-03`**: `HashMap` Entry API probe (prove borrow exclusivity or fall back to `get_or_insert`).
11. **`CORE-GAP-04`**: `HashSet` set algebra design (lazy iterator vs cloned allocation).
12. **`CORE-GAP-05`**: Iterator adapters carrier lifetime probe (`Peekable`, `FilterMap`, `TakeWhile`).
13. **`CORE-GAP-08`**: `split_at_mut` dynamic disjointness probe (prove non-overlapping borrow legality or classify as `LANGUAGE-BLOCKED`).

### Group C: Design-Required Protocols & Compiler Gaps (Phase 4A $\to$ 4C)
14. **`CORE-GAP-12`**: Luna Conversion Protocol mini-design (`From`/`Into` vs `Convert`).
15. **`CORE-GAP-13`**: Default/construction protocol design.
16. **`CORE-GAP-16`**: Formatting & Error model design.
17. **`CORE-GAP-17`**: `Rc<T>` shared ownership & thread-safety evaluation.
18. **`C-GAP-09`**: Float backend semantic completion audit & repair.
19. **`CORE-GAP-18`**: Float math strategy (`core/math` via FFI/libm/intrinsics).

### Group D: Audited & Deferred Domains (Phase 7)
20. **`CORE-GAP-19` / `CORE-GAP-20`**: Atomics & Concurrency RAII wrappers (`Mutex<T>`, `Condvar`, `AtomicBool`). Deferred to **Stage 7 — Phase 7**.

---

## 5. Phase 4 Structure: Three-Stage Execution

Phase 4 is structured into three disciplined sub-phases:

```
Stage 7 / Phase 4: Core Gap Closure

Phase 4A — Semantic Feasibility & Core Protocol Design
  • HashMap Entry feasibility probe (Entry vs get_or_insert fallback)
  • Carrier iterator lifetime probe (Peekable, FilterMap)
  • split_at_mut dynamic disjointness probe
  • Conversion protocol design
  • Default protocol design
  • Formatting & Error protocol design
  • Rc thread-safety model feasibility
  • C-GAP-09: Float backend semantic audit

Phase 4B — Proven Core Surface Closure
  • Vec depth (swap_remove, retain, dedup, extend, resize with contracts)
  • Vec Clone & Eq implementations
  • HashMap Option<V> return semantics
  • HashSet feasible APIs
  • Iterator consumers (position, nth, last, sum) & Range integer coverage
  • Slice sub-slicing (split_at, get, get_mut) & copy_from_slice<T: Copy>
  • Generic sorting (introsort / bounded quicksort with sound moves)
  • Option & Result combinators (unwrap_or_else, or_else, flatten, transpose)
  • Primitive integer utilities (abs, min, max, clamp, checked_*, saturating_*)
  • Memory utilities (mem::swap, mem::replace, mem::take)

Phase 4C — New Foundational Protocols & Float Math
  • C-GAP-09 repair in luna-backend
  • Float math utilities (core/math)
  • Approved conversion protocol
  • Approved default-like protocol
  • Approved formatting & error model
  • Rc<T> if proven sound under concurrency model
```

---

## 6. Canonical Verdict

*   **Stage 7 — Phase 3: Core Stdlib Surface & Depth Audit**: **COMPLETE & FROZEN ✅**
*   **Next Milestone**: **Stage 7 — Phase 4A: Semantic Feasibility & Core Protocol Design 🔄**
