# STDLIB-CORE-PROTOCOLS-DESIGN-01 — Core Standard Library Protocols, Feasibility Proofs & Transition Specification

**Document**: `docs/spec/stdlib-core-protocols-design.md`  
**Milestone**: `Stage 7 — Phase 4A: Semantic Feasibility & Core Protocol Design`  
**Status**: `COMPLETE & FROZEN ✅`  
**Compliance**: Verified against `mellis-grammar`, `luna-semantic-compliance`, `luna-stdlib-compiler-boundary`, `BOX-FROZEN`, and `STD-ARCH-01`.  
**Reference Baseline**: `docs/spec/stdlib-core-depth-audit.md` (Phase 3 Audit Baseline: 26 Canonical Sysroot Providers).

---

## 1. Executive Summary & Strategic Scope

Phase 4 of the Standard Library Core Depth Roadmap transitions Luna from discovery (Phase 3 Audit) to verified implementation. To maintain architectural integrity and prevent semantic regress, **Phase 4A** executes all necessary semantic probes, audits compiler defects, and establishes frozen protocol designs **before** writing any implementation code in Phase 4B or Phase 4C.

This specification documents:
1. **Empirical Feasibility Probes**: Exact test evidence, failure mechanisms, and soundness boundaries for:
   - `HashMap` Entry API and direct fallback methods (`CORE-GAP-03`, `LANGUAGE-GAP-05`)
   - `split_at_mut` dynamic disjointness and formal proof obligations (`CORE-GAP-08`)
   - Carrier iterator provenance retention across 3 adversarial cases (source drop, carrier move, nested carrier mutation) (`CORE-GAP-05`)
   - `HashSet` set algebra (owned v1 vs lazy view boundaries) (`CORE-GAP-04`)
   - `copy_from_slice` alias safety and disjointness proof (`CORE-GAP-09`)
   - `dyn Writer` nested virtual dispatch and UTF-8 sound error model (`CORE-GAP-16A`)
   - Trait coherence and blanket implementation behavior (`CORE-GAP-12`)
2. **Compiler Semantic Audit (`C-GAP-09`)**: A comprehensive multi-layer audit of floating-point support across typechecker, comptime interpreter, optimizer, and LLVM backend, accompanied by normative safe cast and exact float remainder contracts and a platform-FFI float math strategy (`CORE-GAP-18`).
3. **Foundational Protocol Designs**: Normative architectures for Unified Conversion (`Convert<T>` / `TryConvert<T>`), Default Construction (`trait Default`), Formatting (`core/fmt` writer sink with sound `FmtError::InvalidUtf8`), and Minimal v1 Error Interoperability (strictly decoupled from `?`).
4. **Shared Ownership Concurrency Evaluation**: Analysis of `Rc<T>` soundness under Luna's current absence of static `Send`/`Sync` markers, establishing formal deferral to Phase 7.
5. **Core Surface Contract Freeze**: Rigorous pre-implementation contracts for Phase 4B additions (`Vec::resize`, `Vec::dedup`, `copy_from_slice`, `fill`, memory utilities ordering, and unstable sort memory-safety invariants).
6. **Definitive Transition Matrix**: The authoritative categorization of all audit findings into execution targets (`IMPLEMENT-4B`, `APPROVED-4C`, `SAFE-API-VIA-AUDITED-UNSAFE`, `LANGUAGE-BLOCKED`, and `DEFERRED-P7`), with Phase 3 canonical IDs and full capability scopes strictly preserved.

---

## 2. Empirical Feasibility Probe Evidence & Soundness Proofs (WP1)

All feasibility probes were authored as standalone Luna programs in `tests/stdlib_gap_probes/` and executed via the test harness in `crates/luna-driver/tests/stdlib_phase4a_feasibility_tests.rs` (11 tests passed, 0 failed).

### 2.1. Probe 1: `HashMap` Entry API Feasibility (`CORE-GAP-03`)
* **Probe File**: `tests/stdlib_gap_probes/probe_hashmap_entry_feasibility.ln`
* **Test Cases**: `test_probe_1_entry_a_full_entry_api` & `test_probe_1_entry_b_direct_get_or_insert`

#### 2.1.1. Candidate A: By-Value Intermediate Enum (`ENTRY-A`)
The Rust-style `Entry` API requires an intermediate enum:
```luna
enum Entry<K, V> {
    Occupied(&rw V),
    Vacant(&rw HashMap<K, V>, K),
}

impl<K: Hash + Eq, V> Entry<K, V> {
    fn or_insert(self: Self, default_val: V) -> &rw V life_from(self) { ... }
}
```
* **Empirical Result**: **REJECTED BY COMPILER (EXPECTED) ❌**
* **Diagnostic**: `error[E3005]: LocalBorrowEscape: Reference to local variable escapes function scope`
* **Failure Mechanism**:
  A by-value aggregate cannot currently parameterize and transitively expose borrowed provenance originating from an external lifetime domain. Formally registered as **`LANGUAGE-GAP-05`**.
* **Classification**: **`LANGUAGE-BLOCKED (LANGUAGE-GAP-05)`**.

#### 2.1.2. Candidate B: Sound Direct Map Methods (`ENTRY-B`)
Instead of fabricating an unsound intermediate object, entry capabilities are exposed as direct methods on `HashMap<K, V>`:
```luna
impl<K: Hash + Eq, V> HashMap<K, V> {
    pub fn get_or_insert(self: &rw Self, key: K, default_val: V) -> &rw V life_from(self);
    pub fn insert_if_absent(self: &rw Self, key: K, val: V) -> bool;
}
```
* **Empirical Result**: **ACCEPTED & VERIFIED ✅** (Exit code 0, mutated value `400` in-place).
* **Normative Ownership Contract for Phase 4B**:
  * **`get_or_insert(key: K, default_val: V) -> &rw V life_from(self)`**:
    - If `key` absent: `key` and `default_val` become owned by the map; returns `&rw` reference to the newly inserted value.
    - If `key` present: map remains unchanged; supplied `key` and `default_val` remain caller-consumed arguments and their unused owned values are dropped exactly once; returns `&rw` reference to existing value.
    - Returned reference lifetime: `life_from(self)`.
  * **`insert_if_absent(key: K, val: V) -> bool`**:
    - If `key` absent: `key` and `val` become owned by map; returns `true`.
    - If `key` present: map remains unchanged; supplied `key` and `val` are dropped exactly once; returns `false`.

---

### 2.2. Probe 2: `split_at_mut` Dynamic Disjointness (`CORE-GAP-08`)
* **Probe File**: `tests/stdlib_gap_probes/probe_split_at_mut_disjointness.ln`
* **Test Case**: `test_probe_2_split_at_mut_disjointness`

#### 2.2.1. Implementation
```luna
pub fn split_at_mut<T>(s: &rw [T], mid: u64) -> (&rw [T], &rw [T]) life_from(s) {
    dec total_len = s.len();
    if mid > total_len {
        __luna_panic_default();
    }
    unsafe {
        dec raw_ptr = s.as_mut_ptr();
        dec left = mem::slice_from_raw_parts_mut<T>(raw_ptr, mid);
        dec right_ptr = ptr::add_mut<T>(raw_ptr, mid);
        dec right = mem::slice_from_raw_parts_mut<T>(right_ptr, total_len - mid);
        return (left, right);
    }
}
```
* **Empirical Result**: **ACCEPTED & VERIFIED ✅** (Mutated disjoint offsets `buf[0]` and `buf[2]` concurrently; exit code 0).
* **Classification**: **`SAFE-API-VIA-AUDITED-UNSAFE`**.

#### 2.2.2. Formal Soundness Proof Obligations
The safety of `split_at_mut` relies on the following formal mathematical obligations:
```text
Precondition:
    mid <= len

Partition:
    left  = [0, mid)
    right = [mid, len)

Disjointness:
    left ∩ right = ∅

Pointer Validity:
    both left and right pointers derive from the same valid base allocation
    pointer arithmetic (raw_ptr + mid) remains strictly within bounds [0, len]
    no overflow in byte offset computation (mid * sizeof(T))

Lifetime Invariant:
    life(left)  <= life(s)
    life(right) <= life(s)

Exclusivity Capability:
    while either returned &rw slice is live,
    the parent &rw slice s cannot be independently accessed or aliased

Zero-Length Invariants:
    mid = 0       → left = empty (len 0), right = full (len)
    mid = len     → left = full (len), right = empty (len 0)
    len = 0, mid = 0 → both empty (len 0)
    all cases valid and non-aliasing
```

---

### 2.3. Probe 3: Carrier Iterator Lifetimes & Full Adversarial Evidence (`CORE-GAP-05`)
* **Probe Files**: [`probe_carrier_iterator_lifetimes.ln`](file:///d:/fdlang/mellis-rs/tests/stdlib_gap_probes/probe_carrier_iterator_lifetimes.ln) & [`probe_carrier_iterator_adversarial.ln`](file:///d:/fdlang/mellis-rs/tests/stdlib_gap_probes/probe_carrier_iterator_adversarial.ln)
* **Test Cases**:
  - `test_probe_3_carrier_iterator_lifetimes`: Aggregates caching `Option<&T>` compile and yield expected sequence (`ACCEPTED ✅`).
  - `test_probe_carrier_case_a_source_drop_rejected`: Source move/drop while carrier retains provenance $\longrightarrow$ **`REJECTED_AS_EXPECTED ✅`**.
  - `test_probe_carrier_case_b_carrier_move_accepted`: Carrier itself is moved while containing borrowed Item, source remains alive $\longrightarrow$ **`ACCEPTED ✅`** (exit code 0, asserting borrowed item `10`). Borrowed reference in carrier is tied to source provenance, not carrier stack location.
  - `test_probe_carrier_case_c_nested_carrier_mutation_rejected`: Nested carrier/adapter retains provenance, attempt source mutation $\longrightarrow$ **`REJECTED_AS_EXPECTED ✅`**.
* **Classification**: **`PROVEN-FEASIBLE`**.

---

### 2.4. Probe 4: `HashSet` Set Algebra Feasibility (`CORE-GAP-04`)
* **Probe File**: `tests/stdlib_gap_probes/probe_hashset_set_algebra_feasibility.ln`
* **Test Case**: `test_probe_4_hashset_set_algebra`
* **Findings**:
  - Lazy borrowed views are deferred (`LANGUAGE-GAP-04`).
  - Owned algebra `union_owned<T: Clone + Eq + Hash>(&a, &b) -> HashSet<T>` verified (exit code 0). Adopted for Phase 4B.

---

### 2.5. Probe 5: Slice Copy Alias Safety (`CORE-GAP-09`)
* **Probe File**: `tests/stdlib_gap_probes/probe_slice_copy_alias.ln`
* **Test Case**: `test_probe_slice_copy_alias`
* **Findings**:
  - Attempting to simultaneously create mutable and shared slice borrows of the same allocation is rejected by borrowck (`REJECTED_AS_EXPECTED ✅`).
  - Safe Luna cannot alias mutable and shared slices of the same allocation.
  - Therefore, `copy_from_slice(dst: &rw [T], src: &[T])` is guaranteed disjoint by the borrow checker in safe code, making lowering to `memcpy` completely sound.

---

### 2.6. Probe 6: Dyn Writer Nested Virtual Dispatch & Sound Error Model (`CORE-GAP-16A`)
* **Probe File**: `tests/stdlib_gap_probes/probe_dyn_writer_nested_dispatch.ln`
* **Test Case**: `test_probe_dyn_writer_nested_dispatch`

#### 2.6.1. Empirical Verification
Tested full nested virtual dispatch:
`Concrete Display -> Display::fmt(&self, w: &rw dyn Writer) -> dyn Writer.write_utf8(&[u8])`
```luna
enum FmtError {
    InvalidUtf8,
    WriteFailed,
}

trait Writer {
    fn write_utf8(self: &rw Self, bytes: &[u8]) -> Result<(), FmtError>;
}

trait Display {
    fn fmt(self: &Self, w: &rw dyn Writer) -> Result<(), FmtError>;
}
```
* **Empirical Result**: **ACCEPTED & VERIFIED ✅**
  - Compiled and lowered to explicit MVIR `make_trait_object` and `call_virt`.
  - In-place mutation observed (`writer.count == 3 as usize`).
  - Provenance retained and borrow released cleanly; exited code 0.
* **Classification**: **`PROVEN-FEASIBLE`**.
* **Sound UTF-8 Contract**:
  - `String` must always contain valid UTF-8. `[u8]` is arbitrary bytes.
  - Calling `write_utf8(bytes: &[u8])` does NOT assume caller guarantees valid UTF-8.
  - **Normative Rule**: `Writer::write_utf8(bytes)`:
    - If `bytes` is valid UTF-8 $\longrightarrow$ write them.
    - Otherwise $\longrightarrow$ return `Err(FmtError::InvalidUtf8)`.
    - Every implementation of `Writer` (including `impl Writer for String`) must enforce this contract by validating UTF-8 before appending if input lacks type-level proof.
* **Layering & Dependencies**:
  - Sysroot dependency order: `alloc` may depend on `core`; `core` does **not** depend on `alloc`.
  - `StaticBufWriter<N>` is dropped due to absence of const generics in Luna v1.

---

### 2.7. Probe 7: Conversion Protocol Coherence (`CORE-GAP-12`)
* **Probe File**: `tests/stdlib_gap_probes/probe_convert_coherence.ln`
* **Test Case**: `test_probe_conversion_coherence`

Tested 4 canonical coherence configurations:
1. **`CONVERT-COHERENCE-1`**: Generic blanket impl alone (`impl<T> Convert<T> for T`) $\longrightarrow$ **`ACCEPTED`** (compiles cleanly).
2. **`CONVERT-COHERENCE-2`**: Non-overlapping concrete impl (`impl Convert<i32> for bool`) $\longrightarrow$ **`ACCEPTED`** (compiles and executes deterministically).
3. **`CONVERT-COHERENCE-3`**: Concrete impl overlapping reflexive blanket impl (`impl Convert<MyType> for MyType`) alongside blanket $\longrightarrow$ **`REJECTED_AS_EXPECTED`** (rejected by trait coherence).
4. **`CONVERT-COHERENCE-4`**: Method lookup with single applicable impl $\longrightarrow$ **`ACCEPTED`** (deterministic resolution and execution).

* **Canonical Finding**: Luna trait coherence correctly rejects overlapping implementations. Phase 4A does not introduce specialization.
* **Dispatch Finding**: Method dispatch through dot syntax (`x.convert()`) for generic blanket implementations is deferred until generic method resolution is generalized.

---

## 3. Compiler Floating-Point Semantic Audit & Contracts (`C-GAP-09`) & Math Strategy (`CORE-GAP-18`) (WP2)

### 3.1. Multi-Layer Closure Gate for `C-GAP-09` (Phase 4C)

| Layer | Component | Required Closure Behavior |
| :--- | :--- | :--- |
| **Typechecker** | `luna-semantic/typechecker.rs` | Reject bitwise (`&`, `\|`, `^`, `<<`, `>>`) and logical (`&&`, `\|\|`) operations on `f32`/`f64`. |
| **Comptime** | `luna-mvir/interp.rs` | Add `RuntimeValue::Float` branches for `Eq`, `NotEq`, `Rem`, and float-to-int casts. |
| **Optimizer** | `luna-optimizer` | Audit float constant folding to respect IEEE 754 precision and NaN propagation. |
| **Backend** | `luna-backend/llvm_codegen.rs` | 1. Float literals: parse `f32`/`f64` constants without integer pre-parsing.<br>2. Arithmetic: emit `fadd`, `fsub`, `fmul`, `fdiv`.<br>3. Remainder `%`: emit truncated remainder per normative spec.<br>4. Comparisons: emit IEEE 754 predicates (`OEQ`, `UNE`, `OLT`, `OLE`, `OGT`, `OGE`).<br>5. Conversions: `f32` $\leftrightarrow$ `f64`, integer $\to$ float (`sitofp`, `uitofp`).<br>6. Safe float $\to$ integer cast with explicit guards. |

### 3.2. Normative Float-Cast Contract

Backend primitives (`fptosi`/`fptoui`) produce LLVM undefined behavior/poison on out-of-range or NaN values. Safe Luna must not expose backend undefined behavior.

```text
FLOAT-CAST CONTRACT (x as TargetInt):

1. Representable finite value:
   - Truncation toward zero (e.g. 3.7 as i32 -> 3, -3.7 as i32 -> -3).

2. Invalid explicit conversion categories (NaN, ±Infinity, finite out-of-range, negative to unsigned):
   - Comptime evaluation:
     Trigger compile-time evaluation failure / compile-time evaluation diagnostic.
   - Runtime execution:
     Trigger deterministic runtime numeric-conversion trap.
```

### 3.3. Normative Float Remainder Semantics (`%`)

Float remainder `a % b` in Luna is frozen as **truncated remainder**, mathematically defined at the language level independently of backend choice:
- **Finite non-zero operands $x, y$**:
  $$x \% y = x - y \cdot \operatorname{trunc}(x / y)$$
  The result has the same sign as the dividend $x$, and its magnitude is strictly less than $|y|$.
  *(Note: The formula $x - y \cdot \operatorname{trunc}(x/y)$ is a semantic characterization; the compiler and backend are not required to literally execute three floating-point operations. Lowering may use LLVM `frem` or platform `fmod` provided the contract is satisfied, avoiding spurious intermediate rounding).*
- **Boundary & Special cases**:
  - $x \% \pm 0 \longrightarrow \text{NaN}$
  - $\pm \infty \% x \longrightarrow \text{NaN}$
  - $x \% \pm \infty \longrightarrow x$ (for finite $x$)
  - $\pm 0 \% x \longrightarrow \pm 0$ (for $x \neq 0$ and not NaN)
  - $\text{NaN} \% x \longrightarrow \text{NaN}$
  - $x \% \text{NaN} \longrightarrow \text{NaN}$
- **Parity Invariant**: Comptime interpreter, LLVM `frem` lowering, and platform FFI math must all conform to this exact specification.

### 3.4. Float Math Strategy (`CORE-GAP-18`) & Runtime Boundaries
* **Runtime ABI**: Zero additions to `runtime/`. The C runtime remains free of math functions.
* **Math Strategy**: Direct platform FFI. Standard math functions (`sqrt`, `sin`, `cos`, `floor`, `ceil`) in `core/math` declare external C symbols linked against the platform C library (`libm` on POSIX, `ucrt` on Windows).
* **LLVM Intrinsics**: Recognized by LLVM as a backend optimization detail only (e.g. lowering `sqrt` to hardware instructions). This is not an ABI contract.

---

## 4. Foundational Protocol Design Specifications (WP3)

### 4.1. Conversion Protocol (`CORE-GAP-12`)
```luna
module core {
    pub trait Convert<T> {
        pub fn convert(self: Self) -> T;
    }

    pub trait TryConvert<T> {
        type Error;
        pub fn try_convert(self: Self) -> Result<T, Self::Error>;
    }
}
```
* **Disposition**: `APPROVED-4C`.
* Reflexive blanket impl `impl<T> Convert<T> for T` is accepted by coherence, but dot-method dispatch is deferred until generic method resolution is generalized.

---

### 4.2. Default Construction Protocol (`CORE-GAP-13`)
```luna
module core {
    pub trait Default {
        pub fn default() -> Self;
    }
}
```
* **Disposition**: `APPROVED-4C`. Pure library trait without compiler magic.

---

### 4.3. Formatting Protocol Architecture (`CORE-GAP-16A`)
```luna
module core {
    pub enum FmtError {
        InvalidUtf8,
        WriteFailed,
    }

    pub trait Writer {
        pub fn write_utf8(self: &rw Self, bytes: &[u8]) -> Result<(), FmtError>;
    }

    pub trait Display {
        pub fn fmt(self: &Self, w: &rw dyn Writer) -> Result<(), FmtError>;
    }
}
```
* **Disposition**: `APPROVED-4C` (proven via Probe 6).
* Upholds the invariant that `String` contains valid UTF-8 via runtime validation if arbitrary `&[u8]` is supplied.
* Preserves sysroot layering: `alloc` may depend on `core`; `core` does **not** depend on `alloc`.

---

### 4.4. Minimal v1 Error Interoperability Model (`CORE-GAP-16B`)
* **Invariants**:
  - `Error` is an optional diagnostic interoperability protocol.
  - `Error` does **not** participate in `?` propagation (`?` desugars via `Try`/`FromResidual`).
  - `Error` depends on the approved formatting protocol (`CORE-GAP-16A`).
  - Source chaining (`source() -> Option<&dyn Error>`) is deferred until recursive `dyn Error` references are formally verified.
  - No dynamic `String` allocation is required; messages are formatted via `Display`.

---

### 4.5. `Rc<T>` Shared Ownership (`CORE-GAP-17`)
* **Soundness Boundary**: Luna does not currently possess compiler-enforced `Send`/`Sync` marker traits. Exposing `Rc<T>` before thread-confinement boundaries are formalized would permit cross-thread data races.
* **Disposition**: **`DEFERRED TO STAGE 7 — PHASE 7 (CONCURRENCY / SYNC / THREADS)`**.

---

## 5. Core Surface Contract Freeze (WP4)

The following normative contracts are frozen for implementation in **Phase 4B**:

### 5.1. `Vec` Depth (`CORE-GAP-01`, `CORE-GAP-02`)
* `resize(self: &rw Self, new_len: u64, value: T)` requires `T: Clone`.
* `dedup(self: &rw Self)` requires `T: Eq`. Removes consecutive duplicates in-place in $O(n)$ time.
* `extend`, `retain`, `swap_remove`.
* `Clone` and `Eq` trait implementations for `Vec<T>`.

### 5.2. Slice Depth & Bulk Operations (`CORE-GAP-08`, `CORE-GAP-09`)
* `get(self: &Self, index: u64) -> Option<&T>` & `get_mut(self: &rw Self, index: u64) -> Option<&rw T>`.
* `split_at(self: &Self, mid: u64) -> (&[T], &[T])` & `split_at_mut` (`AUDITED-UNSAFE-4B`).
* `copy_from_slice<T: Copy>(dst: &rw [T], src: &[T])`:
  - Requires `T: Copy`.
  - Asserts `dst.len() == src.len()` (panics on mismatch).
  - Borrow checker proves `dst` and `src` cannot alias in safe code; lowers to `memcpy`.
* `fill<T: Clone>(s: &rw [T], value: T)`: Requires `T: Clone`.

### 5.3. Generic Sorting Contract (`CORE-GAP-10`)
* **Contract Specification**:
  ```text
  sort / sort_by:
      in-place
      unstable
      worst-case O(n log n) time
      O(log n) auxiliary space
      no Clone requirement

      inconsistent comparator:
          resulting ordering is unspecified
          memory safety and ownership invariants preserved
  ```
* **Implementation Technique**: In v1, implemented via introsort + sound `ptr::swap` (implementation detail, not part of public API commitment).

### 5.4. Primitive Integer Utilities (`CORE-GAP-14`)
* `min(a, b)`, `max(a, b)`.
* `clamp(val, lo, hi)` with precondition `lo <= hi`.
* `checked_add`, `checked_sub`, `checked_mul`, `checked_div` $\longrightarrow$ return `Option<T>` (`None` on overflow/underflow/div-by-zero).
* `saturating_add`, `saturating_sub`, `saturating_mul` $\longrightarrow$ clamp to `MIN`/`MAX`.
* `abs`: Evaluates according to Luna signed-overflow policy (two's-complement wrapping or trap). Independent safe alternative `checked_abs(x) -> Option<T>` returns `None` on `MIN` and `Some(abs(x))` otherwise.
* Range coverage explicitly includes `u8`–`u64`, `i8`–`i64`, `usize`, and `isize`.

### 5.5. Memory Utilities Dependency Ordering (`CORE-GAP-15`)
* **Phase 4B**: `mem::swap(&rw T, &rw T)` and `mem::replace(&rw T, T) -> T` (caller supplies replacement; no dependencies).
* **Phase 4C**: `mem::take(&rw T) -> T` (deferred to Phase 4C because it requires `T: Default`).

---

## 6. Definitive Classification & Transition Matrix (WP5)

All audit capabilities are formally classified below, strictly preserving Phase 3 canonical IDs and complete scopes:

| Gap ID | Sub-Capability | Classification | Execution Milestone | Soundness / Architectural Rationale |
| :--- | :--- | :--- | :--- | :--- |
| **`CORE-GAP-01`** | `Vec` Depth (`swap_remove`, `retain`, `dedup`, `extend`, `resize`) | `IMPLEMENT-4B` | Phase 4B | Proven core surface; contracts require `T: Clone` and `T: Eq`. |
| **`CORE-GAP-02`** | `Vec` Trait Impls (`Clone`, `Eq`) | `IMPLEMENT-4B` | Phase 4B | `impl<T: Clone> Clone for Vec<T>`, `impl<T: Eq> Eq for Vec<T>`. |
| **`CORE-GAP-03`** | `HashMap` Direct Fallbacks (`get_or_insert`, `insert_if_absent`) | `IMPLEMENT-4B` | Phase 4B | By-value `Entry` blocked by `LANGUAGE-GAP-05`. Direct methods proven and ownership frozen. |
| **`CORE-GAP-04`** | `HashSet` Owned Set Algebra (`union`, `intersection`, `diff`) | `IMPLEMENT-4B` | Phase 4B | Owned set algebra proven. Lazy borrowed views deferred (`LANGUAGE-GAP-04`). |
| **`CORE-GAP-05`** | Iterator Adapters: `Chain` | `IMPLEMENT-4B` | Phase 4B | Sequential iterator composition. |
| | Iterator Adapters: `Peekable` | `IMPLEMENT-4B` | Phase 4B | Provenance retention across 3 adversarial cases proven. |
| | Iterator Adapters: `FilterMap` | `IMPLEMENT-4B` | Phase 4B | Conditional transform adapter. |
| | Iterator Adapters: `TakeWhile` | `IMPLEMENT-4B` | Phase 4B | Predicate-bounded iterator adapter. |
| **`CORE-GAP-06`** | Iterator Consumers (`position`, `nth`, `last`, `sum`) | `IMPLEMENT-4B` | Phase 4B | Pure-Luna iterative consumption algorithms. |
| **`CORE-GAP-07`** | Range Coverage (`u8`–`u64`, `i8`–`i64`, `usize`, `isize`) | `IMPLEMENT-4B` | Phase 4B | Complete integer range coverage. |
| **`CORE-GAP-08`** | Slice Depth: `get`, `get_mut`, `split_at` | `IMPLEMENT-4B` | Phase 4B | Safe sub-slice and indexing methods. |
| | Slice Depth: `split_at_mut` | `AUDITED-UNSAFE-4B` | Phase 4B | Bounds-checked pointer math safe at public API; formal proof obligations frozen. |
| **`CORE-GAP-09`** | Bulk Slice Copying (`copy_from_slice`, `fill`) | `IMPLEMENT-4B` | Phase 4B | Slice aliasing probe proved disjointness; `memcpy` sound for safe code. |
| **`CORE-GAP-10`** | Generic Sorting Contract (unstable in-place sort) | `IMPLEMENT-4B` | Phase 4B | Unspecified ordering on inconsistent comparator; memory safety invariant guaranteed. |
| **`CORE-GAP-11`** | `Option` & `Result` Combinators | `IMPLEMENT-4B` | Phase 4B | `unwrap_or_else`, `or_else`, `flatten`, `transpose`. |
| **`CORE-GAP-12`** | Conversion Protocol (`Convert`, `TryConvert`) | `APPROVED-4C` | Phase 4C | Single-trait duality approved; coherence verified; dot-dispatch deferred. |
| **`CORE-GAP-13`** | Default Construction Protocol (`trait Default`) | `APPROVED-4C` | Phase 4C | Pure library trait design approved for core primitives and containers. |
| **`CORE-GAP-14`** | Integer Utilities: `min`, `max`, `clamp` | `IMPLEMENT-4B` | Phase 4B | Boundary clamping and comparison utilities. |
| | Integer Utilities: `checked_*`, `saturating_*`, `abs`, `checked_abs` | `IMPLEMENT-4B` | Phase 4B | Normative edge-case contracts frozen. |
| **`CORE-GAP-15`** | Memory Utilities: `mem::swap`, `mem::replace` | `IMPLEMENT-4B` | Phase 4B | Caller supplies replacement; move semantics preserved. |
| | Memory Utilities: `mem::take` | `AFTER-DEFAULT / 4C` | Phase 4C | Dependent on `trait Default`. |
| **`CORE-GAP-16A`** | Formatting Protocol (`core/fmt` Writer Sink) | `APPROVED-4C` | Phase 4C | `FmtError::InvalidUtf8` sound contract + `&rw dyn Writer` nested dispatch proven. |
| **`CORE-GAP-16B`** | Error Interoperability Model (`trait Error`) | `APPROVED-4C` | Phase 4C | Minimal v1; decoupled from `?`; diagnostic formatting via `Display`. |
| **`CORE-GAP-17`** | `Rc<T>` Shared Ownership | `DEFERRED-P7` | Phase 7 | Deferred until static `Send`/`Sync` thread-confinement rules exist. |
| **`CORE-GAP-18`** | Float Math Utilities (`core/math`) | `APPROVED-4C` | Phase 4C | Platform C FFI boundary (`libm`/CRT); zero runtime C-ABI additions. |
| **`CORE-GAP-19`** | Atomics (`AtomicBool`, `AtomicI32`, etc.) | `DEFERRED-P7` | Phase 7 | Deferred to Concurrency / Synchronization milestone. |
| **`CORE-GAP-20`** | Mutex / Condvar / Thread Synchronization | `DEFERRED-P7` | Phase 7 | Deferred to Concurrency / Synchronization milestone. |
| **`C-GAP-09`** | Float Compiler Multi-Layer Completion | `COMPILER-GAP-4C` | Phase 4C | Multi-layer closure gate frozen across typechecker, comptime, optimizer, backend. |

---

## 7. Canonical Verdict & Closure Gate

* **Stage 7 — Phase 4A: Semantic Feasibility & Core Protocol Design**: **COMPLETE & FROZEN ✅**
* **Mandatory Closure Gate Verification**:
  - **Borrow / Region**:
    - `Entry` carrier: `BLOCKED` and registered as `LANGUAGE-GAP-05` ✅
    - Direct `HashMap` fallback: `PROVEN` and caller-consumption/drop-once contracts frozen ✅
    - `split_at_mut`: `AUDITED-UNSAFE` with formal mathematical proof obligations frozen ✅
    - Iterator provenance carrier: `PROVEN` across 3 adversarial cases (source drop, carrier move, nested carrier mutation) ✅
    - Set algebra: `OWNED v1` proven and adopted ✅
  - **Core Contracts**:
    - `Vec` depth, `Clone`, `Eq`: `FROZEN` ✅
    - Slice copy/fill: `FROZEN` with alias disjointness verified ✅
    - Sort: `FROZEN` with unstable in-place contract and memory safety preserved on inconsistent comparators ✅
    - Numeric utilities: `FROZEN` with min/max/clamp, checked, saturating, and abs/checked_abs contracts ✅
    - `mem` dependency ordering: `FROZEN` (`swap`/`replace` in 4B, `take` in 4C) ✅
  - **Protocols**:
    - `Convert` / `TryConvert`: `FROZEN` ✅
    - Coherence behavior: `PROVEN` (4 cases verified, overlapping impl rejected) ✅
    - `Default`: `FROZEN` ✅
    - `Writer` / `Display`: `PROVEN` (nested virtual dispatch verified) + `FROZEN` with `FmtError::InvalidUtf8` validation ✅
    - `Error`: Minimal v1 `FROZEN` ✅
    - `Rc<T>`: `DEFERRED-P7` ✅
  - **Floats**:
    - Compiler gap scope: `FROZEN` ✅
    - Comparison semantics: `FROZEN` (IEEE 754 predicates) ✅
    - Remainder semantics: `FROZEN` (exact mathematical truncated remainder spec) ✅
    - Cast semantics: `FROZEN` (comptime diagnostic; runtime numeric-conversion trap) ✅
    - Math FFI boundary: `FROZEN` (platform C FFI, zero runtime additions) ✅
  - **Registry**:
    - Existing Phase 3 `CORE-GAP` IDs and full scopes strictly retained ✅
    - `LANGUAGE-GAP-05` registered ✅
  - **Regression**:
    - Phase 4A probes: 11 passed, 0 failed ✅
    - Workspace test suite: Full suite verified clean ✅
