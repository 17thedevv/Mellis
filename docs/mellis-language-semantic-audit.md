# Mellis Language Semantic Audit (v1.0 Specification Level)

**Scope**: Language Specification & Semantic Interactions (Source-Independent)  
**Status**: Formal Audit Complete  
**Guiding Principle**: *"Two features can each be internally sound in isolation, yet produce soundless behavior or paradoxes when composed."*

---

## 1. Specification Audit Taxonomy

Each language feature and construct is evaluated across **10 Semantic Dimensions**:

1. **Syntax**: Formal grammar representation (`mellis-grammar` SSOT).
2. **Static Semantics**: Scoping, visibility, hygiene, and well-formedness rules.
3. **Type Semantics**: Typing judgments, generic substitution, inference bounds.
4. **Ownership Semantics**: Move, borrow, bitwise copy, drop eligibility.
5. **Evaluation Semantics**: Operational semantics, order of evaluation, sequencing.
6. **Failure / Error Semantics**: Panic behavior, bounds check handling, abort conditions.
7. **Feature Interactions**: Cross-composition with other language constructs.
8. **Ambiguity Cases**: Potential lexical, syntactic, or semantic ambiguities.
9. **Mandatory Invariants**: Invariants that the language guarantees never fail.
10. **Holes / Undefined Areas**: Gaps where language specification currently yields unspecified behavior.

### Status Classifications

*   `DEFINED`: Complete, formal, unambiguous specification exists.
*   `PARTIAL`: Primary semantics defined, but edge cases or boundary conditions are open.
*   `UNDEFINED`: Compiler contains implementation logic, but language specification has not formalized the semantics.
*   `CONTRADICTED`: Two or more specification rules or compiler assumptions make incompatible claims.
*   `IMPLEMENTED`: Semantics are formally defined and verified in the compiler.

---

## 2. Layer-by-Layer Semantic Audit

### Layer 1: Type System

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Primitives (`i32`, `u8`, `f64`, etc.)** | `DEFINED` | Fixed-width, explicitly sized. No implicit widening/narrowing. |
| **References (`&T`, `&rw T`)** | `DEFINED` | Second-class non-nullable pointers with compile-time lifetime/provenance tracking. `&rw T` guarantees exclusive access. |
| **Raw Pointers (`*T`, `*rw T`)** | `IMPLEMENTED` | Second-class untracked addresses (`Copy`). Dereferencing (`*p`), element-stride pointer arithmetic (`p + n`), and calling `unsafe fn` require enclosing `unsafe` context (Rule H). Model B pointer passing is safe. |
| **Generics & Substitution** | `DEFINED` | Monomorphized at compile-time. Generics cannot be dynamic (unless behind `dyn Trait`). Full type-generic substitution occurs before borrow check. |
| **Type Inference** | `DEFINED` | Local bidirectional inference (Hindley-Milner inspired with local constraint solving). Inference never escapes function boundaries. |
| **Coercions & Subtyping** | `IMPLEMENTED` | Subtyping exists exclusively on lifetimes ($\text{longer} <: \text{shorter}$). Unified coercion engine (`coercion::try_coerce`) handles mutability narrowing (`&rw T -> &T`), array-to-slice (`&[T; N] -> &[T]`), and trait object unsizing (`&T -> &dyn Trait`) via explicit MVIR instructions (Rule I). |
| **Recursive Types** | `DEFINED` | Infinite-size struct without indirection is rejected at typecheck. Recursive types must utilize pointer/box indirection. |

---

### Layer 2: Ownership, Destruction & Borrowing

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Move Semantics** | `DEFINED` | Default semantics for non-`Copy` types. Invalidation of source place upon assignment or call. |
| **Copy Semantics** | `PARTIAL` | Identified by `LangItem::Copy`. Bitwise duplication on move. **Contradiction risk**: Interaction with `Drop` (see Section 3). |
| **Shared Borrow (`&T`)** | `DEFINED` | Freezes place from mutation and move while active. Re-borrowing permitted. |
| **Exclusive Borrow (`&rw T`)**| `DEFINED` | Forbids all other borrows (shared or exclusive) and reads of origin place. |
| **Lifetimes & Provenance** | `DEFINED` | CFG-based non-lexical lifetime with multi-path alias set tracking across CFG joins. |
| **Partial Moves** | `PARTIAL` | Moving a field out of a struct leaves other fields accessible. **Hole**: Partial move on types implementing `Drop` is dangerous without a clear specification. |
| **Destruction (`Drop`)** | `DEFINED` | Scope exit invokes `DropFn`. Drop flags track partial initialization/moves across control flow. |

---

### Layer 3: Expressions & Control Flow

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Operators & Precedence** | `DEFINED` | Explicit precedence table in `mellis-grammar`. Left-to-right evaluation order. |
| **Calls & Method Resolution** | `DEFINED` | Auto-deref / auto-ref on receiver up to 1 level for `&` and `&rw`. Exact signature match prioritized over generic candidate. |
| **Field Access & Indexing** | `DEFINED` | Field access tracks place provenance. Array/slice indexing emits bounds check panic on violation. |
| **Pattern Matching (`match`)** | `DEFINED` | Exhaustive match required. Arms use `->`. Decision tree formal exhaustiveness validation. |
| **Try Operator (`?`)** | `DEFINED` | Desugared via `Try::branch` returning `ControlFlow`. Early return via `FromResidual::from_residual`. |
| **Loops & Loop Control** | `DEFINED` | `loop`, `while`. `break` and `continue` strictly checked for target loop depth. |
| **`async` / `await`** | `IMPLEMENTED` | Resumable state machine transformation with state-dispatched drop/cancellation and caller borrow lifecycle (`FutureLoan`). |

---

### Layer 4: Statements & Bindings

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **`dec` vs `dec rw`** | `DEFINED` | `dec` binds immutable place. `dec rw` binds mutable place. Reassignment requires `rw`. |
| **`const` Bindings**         | `IMPLEMENTED` | Compile-time constant evaluation. Admitted via `is_const_evaluable`, sorted topologically, evaluated on MVIR interpreter. Cycles emit `E_CONST_DEPENDENCY_CYCLE`. |
| **Assignment** | `DEFINED` | Requires mutable LHS place. Performs move or copy of RHS. Overwrites destination, invoking drop if previously initialized. |
| **Return** | `DEFINED` | Evaluates return expression, runs drops of all active local scopes in reverse order of declaration, yields value to caller. |

---

### Layer 5: Functions & Signatures

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Parameters & ABI** | `DEFINED` | By-value parameters take ownership. Reference parameters enforce caller lifetime outliving function invocation. |
| **Return References** | `DEFINED` | Lifetime of return reference must be tied to at least one input lifetime parameter (escape analysis rule). |
| **Generic Functions** | `DEFINED` | Fully monomorphized. Type parameters can carry trait bounds. |

---

### Layer 6: Traits & Protocols

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Trait Definitions** | `DEFINED` | Defines method signatures and associated types. Visibility of methods inherits from trait visibility. |
| **Trait Impls** | `DEFINED` | Must provide concrete implementations for all required methods. |
| **Orphan Rule / Coherence** | `IMPLEMENTED` | Strict orphan rule ($\text{Local Trait} \lor \text{Local Nominal Head}$) and unification-based coherence ($\forall (\text{Trait}, \text{SelfType}): \text{at most one applicable impl}$) enforced across local compilation and cross-module injection. |
| **Associated Types** | `IMPLEMENTED` | Syntax (`type Output;`), eager projection normalization, associated equality constraints (`T: Trait<Item = U>`), and cycle detection (`E_ASSOC_TYPE_CYCLE`) fully implemented. |

---

### Layer 7: Modules, Namespaces & Providers

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Provider (`import`)** | `DEFINED` | Selects external/local provider. Does NOT inject symbols into local lexical scope. |
| **Namespace (`module`)** | `DEFINED` | Multiple providers can contribute to same public namespace (e.g. `std::`). |
| **Alias (`using ... as ...`)**| `DEFINED` | Creates purely local alias to namespace. Never exported. Target must be module, not type. |
| **Cross-Module Identity** | `DEFINED` | `CanonicalSymbolId` provides stable canonical declaration identity across compiler module boundaries. |

---

### Layer 8: Compile-Time & Metaprogramming

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **Declarative Macros** | `DEFINED` | Hygenic macro system using `@` metavariables and `=>` rule dividers. |
| **Comptime Blocks (`comptime`)** | `IMPLEMENTED` | Executed inside VM during compile-time. Sandboxed execution with effect containment, affine ownership/drop tracking, and configurable step/recursion limits. |
| **Intrinsics** | `DEFINED` | Dispatched via compiler-owned `IntrinsicKind`. Type info reflection verified. |

---

## 3. The Feature Interaction Audit Matrix (Feature A × Feature B)

The highest-risk vulnerabilities in language design occur when two individually sound features interact.

```
┌───────────────────────────────┬──────────────────────────────────────────────────────────────┬──────────────┐
│ Interaction Pair              │ Semantic Collision / Invariant Question                      │ Status       │
├───────────────────────────────┼──────────────────────────────────────────────────────────────┼──────────────┤
│ 1. Drop × Copy                │ Can a type implement both Drop and Copy?                     │ IMPLEMENTED  │
│ 2. Drop × Partial Move        │ Can fields be moved out of a struct that implements Drop?    │ IMPLEMENTED  │
│ 3. Copy × Move                │ Does generic T move or copy when T: Copy is not known?       │ DEFINED      │
│ 4. Lifetime × Closure         │ Can a closure escape its environment with captured borrows?  │ DEFINED      │
│ 5. Trait × Associated Type    │ How does type inference resolve ambiguous associated types?  │ IMPLEMENTED  │
│ 6. Try (?) × Return           │ Does early return through ? preserve exact drop order?       │ DEFINED      │
│ 7. Try (?) × Drop             │ What happens if Drop::drop triggers during ? early return?   │ DEFINED      │
│ 8. Async × Borrow             │ Can a borrow cross an await point safely?                    │ IMPLEMENTED  │
│ 9. Async × Drop               │ What happens when an incomplete suspended future is dropped? │ IMPLEMENTED  │
│ 10. Macro × Hygiene           │ Do macro-generated identifiers leak into outer scopes?       │ DEFINED      │
│ 11. Comptime × Ownership      │ Can comptime evaluation move runtime-only heap pointers?     │ DEFINED      │
│ 12. Const × Comptime          │ Unified MVIR evaluation with dual admission and strict VM isolation. │ IMPLEMENTED  │
│ 13. Pattern Match × Move      │ Does matching by value partially invalidate the subject?     │ DEFINED      │
└───────────────────────────────┴──────────────────────────────────────────────────────────────┴──────────────┘
```

---

## 4. Deep-Dive on Critical Findings & Holes

### Contradiction 1: `Drop × Copy`
*   **The Conflict**: `Copy` implies trivial, bitwise duplication where neither instance owns unique resources. `Drop` implies a unique resource that must execute custom cleanup logic exactly once.
*   **Frozen Normative Invariant (Rule A)**: **A concrete type cannot satisfy both `Copy` and `Drop`.**
*   **Enforcement Paths**:
    1.  **Direct Impl Collision**: `impl Drop for T` is strictly rejected if `T: Copy` (and vice-versa).
    2.  **Derivation**: `#[derive(Copy)]` on a struct/enum is rejected if the type itself implements `Drop`.
    3.  **Transitive Field Constraints**: If any field $f_i$ of `struct S { f_1: T_1, ..., f_n: T_n }` satisfies `Drop`, `S` cannot satisfy or derive `Copy`.
    4.  **Generic Bounds**: Bound constraints like `where T: Copy + Drop` are impossible for any concrete type; whether rejected at definition-time or instantiation-time is a diagnostic choice separated from the core invariant.

### Contradiction 2: `Drop × Partial Move`
*   **The Conflict**: If a field is moved out of a container whose type implements `Drop`, running the container's destructor at scope exit would execute on a partially deinitialized, corrupted place.
*   **Frozen Normative Invariant (Rule B)**: **A proper subplace of a `Drop` place cannot be moved out. Whole-place move remains legal.**
*   **Exact Place Semantics**:
    1.  `move c.field`: **Strictly rejected** if $c$ (or any proper ancestor of the field) satisfies `Drop`.
    2.  `move c` (Whole-Place Move): **Fully permitted**. Moves entire container, transferring the drop obligation.
    3.  `borrow c.field`: **Allowed**. Borrowing `&c.field` or `&rw c.field` does not deinitialize the place.
    4.  `c.field = new_val`: **Allowed**. Overwriting an existing initialized field replaces it in-place without leaving the container partially moved.
*   **Implementation Status**: `IMPLEMENTED` at the MVIR Place/Move Analysis level. Whole-place moves remain legal, proper-subplace moves beneath a Drop ancestor are rejected, and syntax-specific paths converge into Place semantics.
    *   *Audit Note*: The Place $\rightarrow$ Type resolution currently depends on a `values_to_places` reverse lookup. Consider making Place projection typing deterministic instead of relying on an existing intermediate Value.

### Interaction 8: `Async × Borrow` & Future Storage Semantics (Rule E)
*   **The Conflict**: In `async fn`, local variables live across `.await` suspension points. The compiler lowers the function into a state machine object where cross-suspension locals become fields of the future.
*   **Frozen Normative Invariants (Rule E)**:
    1.  **External Borrows (`&T`, `&rw T`)**: Permitted across `.await` **only if** their provenance is rooted in an outer lifetime parameter provided by the caller (`FutureLoan`), guaranteeing the borrowed target strictly outlives the entire future execution.
    2.  **Self-Referential Borrows**: **Strictly forbidden at the language level** (`LocalLoan` error `E_SELF_REFERENTIAL_BORROW`). A field in the generated future state machine cannot hold a reference pointing to another field within the same future state machine.
    3.  **FutureLoan Lifecycle**: A borrow loaned to a future remains active for the full live duration of the future place under NLL. Moving `fut1 -> fut2` transfers loan ownership. Terminating the future (`await`, `drop`, scope exit) ends the loan.
    4.  *Design Consequence*: Forbidding self-referential futures natively eliminates the requirement for complex, user-facing `Pin` types in Mellis.
*   **Status**: `IMPLEMENTED` (Passes 14/14 P1-B test matrix).

### Interaction 9: `Async × Drop` & State-Dispatched Cancellation (Rule F)
*   **The Conflict**: Dropping an incomplete or suspended future must release heap resources and cascade drop to any active child futures and live locals without double-freeing or leaking.
*   **Frozen Normative Invariants (Rule F)**:
    1.  **Universal Future Drop**: $\forall T, \text{needs\_drop}(\text{Future}<T>) = \text{true}$ unconditionally. Every future owns an `EnvStruct` requiring `{func}_drop`.
    2.  **State-Dispatched Destruction**:
        - `Initial` (State 0): Drops live initialized arguments and pre-await locals, then frees environment.
        - `Suspended(k)` (State $k > 0$): Guards and recursively drops active child future (`child != null`), then drops $\text{DropSet}(k) = \text{LiveAt}(k) \cap \text{InitializedAt}(k) \cap \text{OwnedAt}(k) \cap \neg\text{MovedAt}(k)$, then frees environment.
        - `Completed` (State -1): Fast-paths directly to `free_env`.
        - `Poisoned` (State -2): Reserved; not routed to `free_env` to prevent resource leaks.
    3.  **Move Transfers Destruction**: Moving a future shifts the single destruction obligation; previous owner is deinitialized.
*   **Status**: `IMPLEMENTED` (Passes 15/15 P1-C test matrix).

### Interaction 12: `Const × Comptime` & Compile-Time Evaluation Boundary (Rule G)
*   **The Conflict**: Unifying compile-time evaluation without duplicating evaluators (AST evaluator vs MVIR interpreter) while strictly preventing runtime fallback, infinite compile-time loops, memory leaks, and VM pointer escapes.
*   **Frozen Normative Invariants (Rule G)**:
    1.  **Strict Phase Separation**: $\text{Phase}(\text{Runtime}) \cap \text{Phase}(\text{Comptime}) = \emptyset$. Failures at compile-time (step limits, cycles, divide-by-zero, pointer escapes) terminate compilation immediately; zero runtime fallback.
    2.  **Unified Execution with Dual Admission**: Both `const` and `comptime` lower to MVIR and evaluate via `MvirInterpreter`:
        - `const`: Restricted expression admission (`is_const_evaluable`), pure, non-mutating, non-allocating. Embedded loops/mutations require explicit `comptime { ... }`.
        - `comptime`: Sandboxed execution with effect containment (permits `dec rw`, `while`, moves, drops, local heap allocations; forbids external I/O, `extern fn`, `await`).
    3.  **Const Item Cycle Detection**: Evaluated in topological dependency order. Cycles emit `E_CONST_DEPENDENCY_CYCLE`.
    4.  **VM Memory Isolation & Materialization**: $\text{VM Memory} \not\longrightarrow \text{Runtime}$. VM stack/heap pointers and closures cannot escape (`E_COMPTIME_POINTER_ESCAPE`, `E_COMPTIME_RESOURCE_ESCAPE`). Escaping values must be materialized into runtime-owned representations (immediate SSA constants or immutable `.rodata` storage).
    5.  **Affine VM Lifecycle**: Reading moved values emits `E_USE_OF_MOVED_VALUE`. Drops execute on scope exit and skip moved places. Unfreed allocations emit `E_COMPTIME_RESOURCE_LEAK`.
    6.  **Configurable Boundaries**: Step limit (default: 1,000,000) and recursion limit (default: 512), configurable via `--comptime-steps` and `--comptime-depth`.
*   **Status**: `IMPLEMENTED` (Passes 20/20 P2-A test matrix).

### Interaction 13: `Raw Pointer Safety × Unsafe Boundary` (Rule H)
*   **The Conflict**: Balancing low-level performance and C-ABI interoperability with rigorous memory safety, ensuring raw pointer operations do not degrade safe borrow checking or allow stealth unsoundness.
*   **Frozen Normative Invariants (Rule H)**:
    1.  **Strict Separation of References and Raw Pointers**: References (`&T`, `&rw T`) carry compiler-tracked lifetimes and provenance; dereferencing is safe. Raw pointers (`*T`, `*rw T`) carry zero lifetime/provenance tracking; dereferencing requires an `unsafe` capability.
    2.  **`unsafe` Is an Admission Boundary, NOT a Borrowck Bypass**: `unsafe` admission permits dereferencing, pointer arithmetic, and `unsafe fn` calls, but preserves 100% of borrow checking, move analysis, and lifetime invariants for safe places.
    3.  **Access Mutability vs Object Mutability**: `*T` confers read-only access capability through the pointer; writing through it (`*p = val`) or taking `&rw (*p)` is statically rejected (`E_CANNOT_MUTATE_IMMUTABLE_POINTER`). Target allocation mutability is separate.
    4.  **Raw Aliasing Bypass**: Multiple `*rw T` pointers can alias and mutate identical memory inside `unsafe` without borrowck conflicts.
    5.  **Raw-Derived References**: Re-borrowing `&(*p)` or `&rw (*p)` synthesizes a reference bound to a local validity region, immediately subject to borrowck.
    6.  **Model B Pointer Passing**: Casting to raw pointers (`&x as *T`), copying raw pointers, and passing them to safe functions is safe.
    7.  **Element-Stride Arithmetic**: `p + n` scales by $\text{sizeof}(T)$ via GEP; `p1 - p2` computes signed element distance.
    8.  **Deterministic Comptime Traps**: Comptime VM traps null (`E_NULL_POINTER_DEREFERENCE`), use-after-free (`E_USE_AFTER_FREE`), and out-of-bounds access. Runtime code generation relies on zero-cost hardware protection.
*   **Status**: `IMPLEMENTED` (Passes 20/20 P2-B test matrix in `crates/mellis-driver/tests/p2b_tests.rs`).

### Interaction 14: `Unsize / Dyn Trait Coercion Formalization` (Rule I)
*   **The Conflict**: Formalizing dynamically sized types (`dyn Trait`, `[T]`), fat pointer representations, object safety, and vtable memory layouts without ad-hoc backend assumptions or introducing an implicit `T: Sized` bound solver.
*   **Frozen Normative Invariants (Rule I)**:
    1.  **Direct Value-Position Rejection**: Mellis v1 does not use a `Sized` marker trait in the solver. Instead, unsized types (`dyn Trait`, `[T]`) are strictly rejected in all value positions (let declarations, function parameters, return types, struct fields) with `E_UNSIZED_TYPE_IN_VALUE_POSITION`.
    2.  **Permitted Indirection**: Unsized types are legal exclusively behind references (`&T`, `&rw T`), raw pointers (`*T`, `*rw T`), or `Box<T>`.
    3.  **Unified Coercion Engine**: Coercions (`coercion::try_coerce`) operate distinctly from unification, producing explicit MVIR instructions: `MakeSlice { data_ptr, len }` and `MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym }`. Mutability narrowing (`&rw -> &`) is supported; widening (`& -> &rw`) is rejected.
    4.  **Fat Pointer Layout**:
        - Slices: `{ data: *T, len: i64 }` (element count).
        - Trait Objects: `{ data: *Void, vtable: *Vtable }`.
    5.  **Mellis Trait Object ABI v1**:
        - Layout: `[ drop_glue, size, align, method_0, method_1, ... ]`.
        - Method slots follow trait declaration order.
        - Destructor dispatch (`DropVirt`) guards against `null` drop glue before indirect call.
        - Virtual calls (`CallVirt`) offset method slots by $+3$.
    6.  **Object Safety**: Trait objects require non-generic methods, an explicit receiver (`self`, `&self`, `&rw self`), no unboxed `Self` return, and no `Self` in non-receiver parameter positions (`E_TRAIT_NOT_OBJECT_SAFE`).
    7.  **v1 Restrictions**: Single trait only; associated types on `dyn` rejected (`E_DYN_ASSOCIATED_TYPE_UNSUPPORTED`); supertrait dyn composition rejected (`E_DYN_SUPERTRAIT_UNSUPPORTED`); `Box<dyn Trait>` and `Box<[T]>` rejected (`E_UNSUPPORTED_BOX_UNSIZED`).
    8.  **Monomorphization Rooting**: `MonoCollector` roots all concrete methods and drop implementations for types undergoing `ConcreteToDyn` coercion.
*   **Status**: `IMPLEMENTED` (Passes 22/22 P3 test matrix in `crates/mellis-driver/tests/p3_tests.rs`).

### Finding 5: Associated Type Equality & The Monomorphization Barrier (Rule C)
*   **Critical Soundness Impact**: The `Try` protocol fundamentally depends on associated types:
    $$\text{Try} \longrightarrow \text{Self::Output}, \text{Self::Residual} \longrightarrow \text{FromResidual}$$
*   **Frozen Normative Invariant (Rule C)**: **Associated type projections may remain symbolic during generic analysis, but no unresolved projection may cross the monomorphization boundary into MVIR/backend.**
*   **Lifecycle**: Projections normalize eagerly when concrete types are known. At the monomorphization boundary, zero symbolic projections, generic parameters, or inference variables are allowed.
*   **Implementation & Verification**:
    - **Cross-Module AST Decoupling**: Impl substitutions use lowered `impl_self_types` and `impl_generic_params` registered in `SemanticTables` and exported via `ProviderInterface` (satisfying Compiler Architecture Rules 3 & 5).
    - **Normalization Engine**: Structural pattern matching (`match_types`) recursively instantiates generic projections without inspecting AST arena declarations.
    - **Cycle Detection**: Stack-based projection cycle detection actively guards against infinite normalization loops, emitting `E_ASSOC_TYPE_CYCLE`.
    - **Associated Type Equality Bounds**: Trait bounds with associated type constraints (`T: Iterator<Item = i32>`) validate equality against candidate implementations, emitting `E_ASSOCIATED_TYPE_MISMATCH` on violation.
    - **Monomorphization Barrier**: `is_monomorphic` enforces the strict barrier rejecting any unresolved projections or poisoned error types before MVIR lowering.

### Finding 6: Trait Coherence & The Orphan Rule (Rule P1-A)
*   **Critical Soundness Impact**: Without strict coherence and orphan invariants, different modules or generic instantiations can declare conflicting trait implementations, leading to nondeterministic dispatch, silent overwrites, or broken canonical module isolation.
*   **Frozen Normative Invariants (Rule P1-A)**:
    1.  **Coherence Invariant**:
        $$\forall (\text{Trait}, \text{SelfType}): \text{at most one applicable impl}$$
        - Coherence is evaluated via transitive pattern unification rather than symbol equality:
          $$\text{Impl}_A \text{ overlaps } \text{Impl}_B \iff \text{Trait}(A) == \text{Trait}(B) \land \text{CanUnify}(\text{SelfType}(A), \text{SelfType}(B))$$
        - Direct duplicate impls (`impl Trait for S` + `impl Trait for S`), generic-vs-concrete overlaps (`impl<T> Trait for S<T>` + `impl Trait for S<i32>`), and two-generic overlaps (`impl<T> Trait for S<T>` + `impl<U> Trait for S<U>`) are strictly rejected with `E_CONFLICTING_TRAIT_IMPL`.
        - Distinct concrete generic instantiations (`impl Trait for S<i32>` + `impl Trait for S<f64>`) are non-overlapping and permitted.
    2.  **Orphan Rule Invariant**:
        $$\text{Valid}(\text{impl Trait for SelfType}) \iff \text{Local}(\text{Trait}) \lor \text{Local}(\text{NominalHead}(\text{SelfType}))$$
        - Primitives, tuples, raw pointers, references, and types without a nominal head cannot satisfy `Local(NominalHead)`.
        - Generic type arguments do not confer locality: `impl ForeignTrait<LocalType> for ForeignType<LocalType>` is rejected.
        - Inherent impls (`impl SelfType { ... }`) require $\text{Local}(\text{NominalHead}(\text{SelfType}))$.
        - Violations emit `E_ORPHAN_IMPL`.
*   **Cross-Provider Guarantees (Non-Negotiables)**:
    - **No silent overwrite**: Ingestion refuses to overwrite existing impls.
    - **No "first impl wins"**: Overlapping imported impls produce a compilation error.
    - **No fallback implementation**: Dispatch never falls back to an arbitrary impl if ambiguity is possible.
    - **No provider-local identity guessing**: Canonical declarations use globally stable `CanonicalSymbolId`.
*   **Implementation & Verification**:
    - Unified pattern unification engine in `crates/mellis-semantic/src/coherence.rs`.
    - Integrated into local compiler Pass 1.8 (`TypeChecker`) and cross-module interface loader (`ModuleRegistry::inject_into_ctx`).
    - Full 15-case test matrix passing in `crates/mellis-semantic/tests/p1a_tests.rs`.

---

## 5. Prioritized Action Roadmap Post-Audit

The language roadmap is restructured into four strict priority tiers:

```
Language Specification Roadmap
│
├── P0 (Soundness Foundations — Prerequisite to any code)
│   ├── Drop × Copy (Total mutual exclusivity across direct & indirect paths)
│   ├── Drop × Partial Move (Place-level rejection of subplace moves)
│   └── Associated Type Equality & Projection (Normalization for Try & traits)
│
├── P1 (Protocols & Asynchronous Mechanics)
│   ├── Trait Coherence & Orphan Rules (Cross-provider impl uniqueness)
│   ├── Async × Borrow (Future storage semantics & self-reference ban)
│   └── Async × Drop (Cancellation & cleanup of suspended state machines)
│
├── P2 (Compile-Time & Low-Level Foundations)
│   ├── Const × Comptime (Rule G: IMPLEMENTED)
│   └── Raw Pointer Safety & Unsafe Boundary (Rule H: IMPLEMENTED)
│
└── P3 (Polymorphism & Coercions)
    └── Unsize / Dyn Trait Coercion Formalization (Rule I: IMPLEMENTED)
```

