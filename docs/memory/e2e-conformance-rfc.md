# Mellis Compiler RFC: End-to-End Conformance & Executable Verification (P7 Hardened v2)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P7 Hardened v2)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P7)
- **Scope**: 12 Tier-1 Conformance Suites, Mandatory Cross-Phase Semantic Trace, Negative Pipeline Invariants, Optimization Equivalence, Golden Test Scenarios, Executable Verification
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Tier 1 Conformance & Test Invariants ("How do we prove the entire memory foundation works end-to-end?")

---

## 1. Architectural Purpose & Boundaries

P7 does **NOT** design or alter language semantics.
P7 proves that the frozen contracts from **P0, P1, P2, P3, P4, P5, and P6** are faithfully realized, preserved, and executed across the entire compiler pipeline down to native machine execution:

$$\text{Source (.ms)} \xrightarrow{\text{AST}} \text{Semantic} \xrightarrow{\text{P5 MVIR}} \text{P6 Borrowck} \xrightarrow{\text{LLVM}} \text{.exe Binary} \xrightarrow{\text{P4 Runtime}} \text{Execution Behavior}$$

> **P7 Core Invariant (Cross-Phase Semantic Fidelity)**:
> An event sequence emitted in source code must retain its exact semantic identity as it crosses every compiler phase.
> No compiler pass may silently degrade, merge, or alter memory and ownership lifecycles into untracked generic operations.

---

## 2. Verification Harness & Testing Hierarchy

Mellis establishes a four-tier testing hierarchy to audit memory correctness:

1. **Phase Unit Tests (`cargo test -p mellis-*`)**:
   Target-isolated unit tests verifying layout algebra in `mellis-common`, type resolution in `mellis-semantic`, MVIR verifier rules in `mellis-optimizer`, and dataflow lattices in `mellis-borrowck`.
2. **Compiler Diagnostic & Pipeline Invariant Tests (`mellis-driver`)**:
   Asserts exact diagnostic codes (`E3001` through `E3005`), primary spans, and related spans.
   **Negative Pipeline Invariant**:
   $$\text{Semantic Rejection} \implies \text{Diagnostic Emitted} \ \land \ \text{Compilation Failure} \ \land \ \text{NO valid executable binary generated}$$
3. **Cross-Phase Semantic Trace Tests**:
   Audits intermediate representation dumps across AST $\to$ MVIR $\to$ LLVM to prove semantic fidelity and verify the absence of legacy or generic bypass instructions.
4. **Executable Smoke & Optimization Tests (`.ms` $\to$ `.exe` Execution)**:
   Compiles `.ms` programs across optimization levels (`-O0`, `-O2`, `-O3`), links against `mellis-runtime.lib`, executes the resulting native binary, and verifies identical observable runtime behavior and exit code 0.

---

## 3. The 12 Tier-1 Conformance Suites (Suites A through L)

The 12 suites cover all frozen normative invariants and representative boundary cases across Tier 1:

### Suite A: Layout Model Conformance (P1)
- **A.1 Power-of-2 Alignment**: Asserts rejection of $align == 0$, $align == 3, 5, 7$, and acceptance of powers of two up to 4096.
- **A.2 Stride & Sizing**: Asserts $\text{stride}(T) = \text{round\_up}(\text{size}(T), \text{align}(T))$.
- **A.3 Overflow Guards**: Verifies checked modulo padding formula; tests `LayoutError::ArithmeticOverflow` on boundary values ($usize::MAX - 3$).
- **A.4 Struct Composition (`extend`)**: Asserts sequential field offsets and trailing padding rounding.
- **A.5 Array Composition (`repeat`)**: Asserts checked multiplication $n \times \text{stride}(T)$; asserts `repeat(0)` produces $size = 0, stride = 0, align = align(T)$ for any valid alignment.
- **A.6 Representability Limit**: Asserts rejection of layouts where $size > \text{isize::MAX as usize}$ (`LayoutError::ExceedsAddressSpaceLimit`).

### Suite B: Raw Allocation & Deallocation Conformance (P0, P3, P4)
- **B.1 Allocation Guarantee**: `__mellis_alloc(64, 16)` returns non-null pointer with $ptr \pmod{16} == 0$.
- **B.2a Typed Uninitialized Place**: Reading an uninitialized local variable or moved place is statically rejected by Borrowck (`E3001`).
- **B.2b Raw Uninitialized Storage**: Reading raw storage via an unsafe pointer before initialization is accepted syntactically inside `unsafe`, but classified as semantic Undefined Behavior in the memory model.
- **B.3 Deallocation Contract**: `__mellis_dealloc(ptr, 64, 16)` successfully releases storage.
- **B.4 Allocator Instance Matching**:
  - Statically provable mismatch $\implies$ verifier rejects at compile time.
  - Runtime-unknown allocator identity $\implies$ preserved as runtime precondition for safe container wrappers.

### Suite C: Reallocation Conformance (P0, P3, P4, P5)
- **C.1 Buffer Growth**: Reallocating 64 bytes to 128 bytes preserves original bytes $[0, 64)$ byte-for-byte.
- **C.2 Buffer Shrinkage**: Reallocating 128 bytes to 32 bytes preserves $[0, 32)$ byte-for-byte.
- **C.3 Alignment Preservation**: Asserts $\text{new\_align} == \text{old\_align}$.
- **C.4 Provenance Invalidation**: Accessing the old pointer variable after reallocation is flagged as a temporal violation (`Use-After-Realloc`).
- **C.5 Realloc Failure Atomicity**:
  - P3 Fallible Allocator Test: Reallocation failure returns `Err(AllocError::OutOfMemory)` and proves old allocation remains valid, untouched, and owned by caller.
  - P4 Infallible Global Allocator Test: Allocation failure aborts deterministically without freeing the old allocation before abort.

### Suite D: Zero-Sized Types (ZST) Conformance (P0, P1, P3, P4)
- **D.1 Alignment Matrix**: Tests `__mellis_alloc(0, align)` across alignments $align \in \{1, 2, 4, 8, 16, 64, 256, 4096\}$; asserts $ptr \ne \text{null}$ and $ptr \pmod{align} == 0$ without physical heap allocation.
- **D.2 ZST Deallocation No-Op**: `__mellis_dealloc(ptr, 0, align)` returns immediately as a guaranteed no-op.
- **D.3 ZST Realloc Transitions**:
  - `realloc(zst_ptr, 0, align, 100)`: Allocates fresh 100-byte buffer.
  - `realloc(buf, 100, align, 0)`: Releases buffer and returns canonical ZST pointer.
  - `realloc(zst_ptr, 0, align, 0)`: Returns canonical ZST pointer as no-op.
- **D.4 ZST Pointer Arithmetic**: For $size = 0$, $ptr + n$ yields identical physical address (`address(p + n) == address(p)`).
- **D.5 ZST Logical Position**: Iteration over a slice of ZST elements advances logical element index without altering physical displacement.

### Suite E: Move & Reinitialization Conformance (P5, P6)
- **E.1 Move-Out Semantics**: Moving a non-`Copy` variable consumes the source place, transitioning it to `Moved`.
- **E.2 Use-After-Move Prevention**: Subsequent read of a moved place emits `DiagnosticCode::UseAfterMove` (`E3001`).
- **E.3 Full Reinitialization**: Assigning a new value to a moved place transitions it to `Initialized` via `Initialize`, permitting subsequent reads without drop.
- **E.4 Write Assignment to Initialized Place**: Assigning to an already `Initialized` place triggers destructor for old value before writing new value (`Write`).

### Suite F: Partial Moves & Drop Flags Conformance (P5, P6)
- **F.1 Partial Move Allowed on Aggregate Types**: For struct without user Drop (`struct Pair { a: String, b: String }`), `let x = p.a` leaves `p.b` initialized and `p` partially moved.
- **F.2 Partial Move Forbidden Under Drop (`E3002`)**: For type implementing `Drop`, moving a field emits `DiagnosticCode::PartialMoveUnderDrop` (`E3002`).
- **F.3 Multi-Field Drop Elaboration**: Moving `p.a` and `p.c` in a 3-field struct leaves `p.b` initialized; scope exit drops `p.b` only, skipping `p.a` and `p.c`.
- **F.4 Dynamic Drop Flag Execution**: Conditional moves across branches correctly toggle 1-bit drop flags, executing drops only on the surviving path. Both execution branches are verified at runtime.

### Suite G: Borrow Rules & Overlap Conformance (P6)
- **G.1 Concurrent Shared Borrows**: Multiple `&x` references coexist without diagnostic.
- **G.2 Exclusive Mutable Borrow**: Creating `&rw x` while another `&x` or `&rw x` is active emits `DiagnosticCode::BorrowConflict` (`E3003`).
- **G.3 Move While Borrowed**: Attempting to move out of `x` while `&x` or `&rw x` is active emits `DiagnosticCode::BorrowConflict` (`E3003`).
- **G.4 Hierarchical Overlap Projections**:
  - Disjoint fields (`x.a` vs `x.b`): Borrowing `&rw x.a` and `&rw x.b` concurrently succeeds.
  - Prefix overlap (`x` vs `x.a.b`): Borrowing `&rw x` conflicts with active `&x.a.b`.
  - Dynamic Index Overlap: When slice indices cannot be statically proven distinct (`x[i]` vs `x[j]`), borrowck conservatively flags mutable access conflicts.
- **G.5 Access through Mutable Loan**: Reading and writing through `*r` where `r: &rw T` is fully valid.

### Suite H: Realloc with Active Borrow Invalidation (P2, P6)
- **H.1 Stale Loan Detection**: Creating a reference into a buffer (`&buf[0]`) and attempting to execute `Realloc` across that loan's lifetime is rejected with `DiagnosticCode::BorrowConflict` (`E3003`).

### Suite I: Pointer & Provenance Conformance (P2)
- **I.1 One-Past Bounds**:
  - **I.1a Statically Provable Dereference**: Statically proven dereference of one-past pointer $base + size$ is rejected by the verifier/borrowck.
  - **I.1b Dynamic Unsafe Pointer**: Out-of-bounds pointer created dynamically in `unsafe` compiles, but dereferencing at runtime is classified as semantic Undefined Behavior.
- **I.2 Integer to Pointer Cast**: `addr as *rw T` synthesizes `Wildcard` provenance and is strictly gated inside `unsafe`.
- **I.3 Pointer Subtraction Soundness**: Subtracting pointers from identical allocation yields element distance; subtracting across different allocations is rejected.
- **I.4 Pointer Equality**: Pointers sharing address but having distinct `AllocId` compare as `false`.

### Suite J: Runtime ABI & Symbol Conformance (P4)
- **J.1 Canonical Symbol Exports**: Verifies `mellis-runtime.lib` exports `__mellis_alloc`, `__mellis_dealloc`, `__mellis_realloc`. (Sentinel is an internal implementation artifact, not a public ABI symbol requirement).
- **J.2 Strict ABI-NO-LIBC Audit**: Audits generated user object files (`.obj`):
  - Must reference exclusively `__mellis_alloc`, `__mellis_dealloc`, `__mellis_realloc`.
  - Must contain **ZERO** references to raw libc `malloc`, `calloc`, `realloc`, `free`.
- **J.3 Calling Convention Compliance**: Verifies Microsoft x64 calling convention compliance and shadow space allocation through LLVM lowering and multi-argument invocation tests.

### Suite K: LLVM Executable End-to-End Tests (Executable Tests)
- **K.1 Native Compilation & Execution**: Compiling `.ms` programs to native `.exe` using `mellis-driver`.
- **K.2 Heap Roundtrip**: Allocates heap storage, writes structured payload, verifies contents, deallocates, exits with 0.
- **K.3 Drop Order Execution**: Verifies the destruction order mandated by the Mellis Language Specification (normative reverse-declaration order).
- **K.4 Optimization Equivalence**: Programs compiled with `-O0`, `-O2`, and `-O3` must produce identical observable behavior (exit codes, drop counts, output data).

### Suite L: Regression & Diagnostic System v1.0 Alignment
- **L.1 Complete Diagnostic Coverage**: Conformance tests verifying all error conditions emit typed codes:
  - `E3001`: `UseAfterMove`
  - `E3002`: `PartialMoveUnderDrop`
  - `E3003`: `BorrowConflict`
  - `E3004`: `MissingReturnValue`
  - `E3005`: `LocalBorrowEscape`
  - `E2023`: `CannotMutateImmutable`
  - `E2025`: `UnsafeOperationOutsideUnsafe`
  - `E2026`: `InvalidCast`

---

## 4. Mandatory Cross-Phase Semantic Trace Test

To prove that semantic events are not degraded into untracked generic operations, P7 mandates a dedicated **Cross-Phase Trace Test**:

```rust
// cross_phase_trace.ms
struct Payload { val: i64 }
impl Drop for Payload {
    fn drop(&rw self) { ... }
}

fn test_lifecycle() {
    dec p = alloc_payload(Payload { val: 42 }); // Alloc + Initialize
    dec moved_p = p;                            // MoveOut
    // Scope exit: DropInPlace + Dealloc
}
```

### Audit Invariants Across Phases:
1. **AST / HIR Phase**: Records the source-level allocator operation and scope boundary.
2. **Semantic Lowering Phase**: Identifies the operation as an allocation semantic event.
3. **MVIR Phase Dump Invariants**:
   - **MUST CONTAIN**:
     - `Instruction::Alloc`
     - `Instruction::Initialize`
     - `Instruction::MoveOut`
     - `Instruction::DropInPlace`
     - `Instruction::Dealloc`
   - **MUST NOT CONTAIN**:
     - `Instruction::BoxNew`
     - `Instruction::BoxFree`
     - `Instruction::HeapAlloc`
     - Generic `Instruction::CallDirect` for allocator operations (e.g. `CallDirect(__mellis_alloc)`)
4. **Borrowck Phase**: Verifies place tree transitions:
   - `p`: `Uninitialized -> Initialized -> Moved`
   - `moved_p`: `Uninitialized -> Initialized -> Dropped`
5. **LLVM IR Phase**: Codegen dump contains:
   - `call ptr @__mellis_alloc`
   - `store`
   - `call void @DropGlue_Payload`
   - `call void @__mellis_dealloc`
6. **Runtime Binary**: Executable runs, executes drop, releases heap, and exits with code 0.

---

## 5. Golden Semantic Scenarios

### Golden Scenario 1: Compiler Memory Allocation & Deallocation
```rust
// golden_scenario_1.ms
// Tests compiler-driven allocation without direct FFI bypass
fn main() -> i32 {
    dec raw_ptr: *rw u8 = GlobalAlloc::allocate(Layout::new(16, 8));
    unsafe {
        dec int_ptr = raw_ptr as *rw i64;
        *int_ptr = 42;
        dec val = *int_ptr;
        if val != 42 { return 1; }
    }
    GlobalAlloc::deallocate(raw_ptr, Layout::new(16, 8));
    return 0;
}
```
*Proof Goal*: Audits MVIR dump to prove emission of `Instruction::Alloc` and `Instruction::Dealloc` (verifying absence of `CallDirect`), lowering to `@__mellis_*`, and native execution to exit 0.

### Golden Scenario 2: ZST Allocation & Scale-by-Zero Arithmetic
```rust
// golden_scenario_2.ms
struct Empty {}

fn main() -> i32 {
    dec a = Empty {};
    dec p1 = &a as *Empty;
    unsafe {
        dec p2 = p1 + 5;
        // Verify physical address invariance under ZST arithmetic
        if (p2 as usize) != (p1 as usize) { return 1; }
    }
    return 0;
}
```
*Proof Goal*: Verifies physical address invariance under ZST arithmetic and compiler lowering.

### Golden Scenario 3: Multi-Field Partial Move & Field-Sensitive Drop
```rust
// golden_scenario_3.ms
struct Resource { id: i32 }
impl Drop for Resource {
    fn drop(&rw self) {
        g_destructor_count = g_destructor_count + 1;
    }
}

struct Triple {
    a: Resource,
    b: Resource,
    c: Resource,
}

fn main() -> i32 {
    dec t = Triple {
        a: Resource { id: 1 },
        b: Resource { id: 2 },
        c: Resource { id: 3 },
    };
    dec r_a = t.a; // Move t.a
    dec r_c = t.c; // Move t.c
    // Scope exit: drops t.b, r_a, r_c. Skips t.a and t.c.
    return 0;
}
```
*Proof Goal*: Proves Place State Tree tracking for multi-field partial moves and field-sensitive drop elaboration with observable destructor counts.

### Golden Scenario 4: Reallocation Primitive with Data Preservation
```rust
// golden_scenario_4.ms
fn main() -> i32 {
    dec p1 = GlobalAlloc::allocate(Layout::new(8, 8));
    unsafe {
        *(p1 as *rw i64) = 123456789;
    }
    // Lowers to MVIR Instruction::Realloc (verifying absence of CallDirect in MVIR)
    dec p2 = GlobalAlloc::reallocate(p1, Layout::new(8, 8), 64);
    unsafe {
        dec val = *(p2 as *const i64);
        if val != 123456789 { return 1; }
    }
    GlobalAlloc::deallocate(p2, Layout::new(64, 8));
    return 0;
}
```
*Proof Goal*: Audits MVIR dump to prove emission of `Instruction::Realloc` (and absence of `CallDirect`), byte preservation, and clean deallocation.

### Golden Scenario 5: Conditional Move & Observable Drop Counter
```rust
// golden_scenario_5.ms
dec rw g_drops: i32 = 0;

struct Tracked { val: i32 }
impl Drop for Tracked {
    fn drop(&rw self) {
        g_drops = g_drops + 1;
    }
}

fn consume(t: Tracked) {
    // consumes t; drops t at consume scope exit
}

fn test_conditional(flag: bool) -> i32 {
    dec item = Tracked { val: 10 };
    if flag {
        consume(item); // moved on true branch
    }
    // join point: drop flag dictates whether item is dropped here
    return 0;
}

fn main() -> i32 {
    // Execution Path 1: flag = true -> consume drops item once
    g_drops = 0;
    test_conditional(true);
    if g_drops != 1 { return 1; }

    // Execution Path 2: flag = false -> scope exit drops item once
    g_drops = 0;
    test_conditional(false);
    if g_drops != 1 { return 2; }

    return 0;
}
```
*Proof Goal*: Executes both CFG branches, proving dynamic drop flag correctness via observable destructor counters.

---

## 6. Summary & Transition to P8 (Tier 1 Freeze)

Upon completing and verifying the 12 Conformance Suites (A through L), the Cross-Phase Semantic Trace, the Optimization Equivalence Suite, and the 5 Golden Scenarios:
1. Every semantic invariant across P0, P1, P2, P3, P4, P5, and P6 is proven operational with full pipeline fidelity.
2. Negative tests prove invalid programs never emit valid executables.
3. No raw libc symbols leak into generated code.
4. Intermediate representation absence checks guarantee legacy monolithic instructions (`BoxNew`, `BoxFree`, `HeapAlloc`, generic `CallDirect`) are eliminated.
5. **Tier 1 (Allocation & Memory Layout Foundation)** is formally frozen (P8).
6. The compiler is completely ready to implement **Tier 2: Primitive Heap Containers (`Box<T>`, `Unique<T>`, `RawVec<T>`)** followed by **Tier 3: User Collections (`Vec<T>`, `String`)**.
