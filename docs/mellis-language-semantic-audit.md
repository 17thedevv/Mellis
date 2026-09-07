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
| **Raw Pointers (`*T`, `*rw T`)** | `PARTIAL` | Syntax and layout defined. **Hole**: Dereferencing rules, safety boundary (`unsafe` blocks), and provenance validity rules for raw pointers are not yet formally specified. |
| **Generics & Substitution** | `DEFINED` | Monomorphized at compile-time. Generics cannot be dynamic (unless behind `dyn Trait`). Full type-generic substitution occurs before borrow check. |
| **Type Inference** | `DEFINED` | Local bidirectional inference (Hindley-Milner inspired with local constraint solving). Inference never escapes function boundaries. |
| **Coercions & Subtyping** | `PARTIAL` | Subtyping exists exclusively on lifetimes ($\text{longer} <: \text{shorter}$). Coercion exists for `&rw T -> &T`. **Hole**: Unsize coercions (array to slice, struct to `dyn Trait`) are partially implemented in backend but lack unified specification. |
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
| **`async` / `await`** | `PARTIAL` | Resumable state machine transformation. **Hole**: Destruction of pending futures holding references across await points (see Section 3). |

---

### Layer 4: Statements & Bindings

| Feature | Status | Core Specification & Findings |
| :--- | :--- | :--- |
| **`dec` vs `dec rw`** | `DEFINED` | `dec` binds immutable place. `dec rw` binds mutable place. Reassignment requires `rw`. |
| **`const` Bindings** | `PARTIAL` | Compile-time constant evaluation. **Ambiguity**: Boundary between `const` expression evaluation and `comptime { ... }` blocks. |
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
| **Orphan Rule / Coherence** | `PARTIAL` | **Hole**: Cross-module orphan rule (can module B implement Trait from module A for Type from module C?) is not formally stated in grammar/reference. |
| **Associated Types** | `PARTIAL` | Syntax exists (`type Output;`). **Hole**: Associated type equality constraints in `where` clauses (`T::Output == U`) need formal unification semantics. |

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
| **Comptime Blocks (`comptime`)** | `PARTIAL` | Executed inside VM during compile-time. **Hole**: Side-effect containment, memory allocation limits, and compile-time ownership tracking. |
| **Intrinsics** | `DEFINED` | Dispatched via compiler-owned `IntrinsicKind`. Type info reflection verified. |

---

## 3. The Feature Interaction Audit Matrix (Feature A × Feature B)

The highest-risk vulnerabilities in language design occur when two individually sound features interact.

```
┌───────────────────────────────┬──────────────────────────────────────────────────────────────┬──────────────┐
│ Interaction Pair              │ Semantic Collision / Invariant Question                      │ Status       │
├───────────────────────────────┼──────────────────────────────────────────────────────────────┼──────────────┤
│ 1. Drop × Copy                │ Can a type implement both Drop and Copy?                     │ CONTRADICTED │
│ 2. Drop × Partial Move        │ Can fields be moved out of a struct that implements Drop?    │ CONTRADICTED │
│ 3. Copy × Move                │ Does generic T move or copy when T: Copy is not known?       │ DEFINED      │
│ 4. Lifetime × Closure         │ Can a closure escape its environment with captured borrows?  │ DEFINED      │
│ 5. Trait × Associated Type    │ How does type inference resolve ambiguous associated types?  │ PARTIAL      │
│ 6. Try (?) × Return           │ Does early return through ? preserve exact drop order?       │ DEFINED      │
│ 7. Try (?) × Drop             │ What happens if Drop::drop triggers during ? early return?   │ DEFINED      │
│ 8. Async × Borrow             │ Can a borrow cross an await point safely?                    │ PARTIAL      │
│ 9. Async × Drop               │ What happens when an incomplete suspended future is dropped? │ PARTIAL      │
│ 10. Macro × Hygiene           │ Do macro-generated identifiers leak into outer scopes?       │ DEFINED      │
│ 11. Comptime × Ownership      │ Can comptime evaluation move runtime-only heap pointers?     │ DEFINED      │
│ 12. Const × Comptime          │ Is const an alias for comptime or a distinct phase?          │ UNDEFINED    │
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

### Hole 3: `Async × Borrow` & Future Storage Semantics
*   **The Conflict**: In `async fn`, local variables live across `.await` suspension points. The compiler lowers the function into a state machine object where cross-suspension locals become fields of the future.
*   **Future Storage Semantics**:
    1.  **External Borrows (`&T`, `&rw T`)**: Permitted across `.await` **only if** their provenance is rooted in an outer lifetime parameter provided by the caller (guaranteeing the borrowed target strictly outlives the entire future execution).
    2.  **Self-Referential Borrows**: **Strictly forbidden at the language level**. A field in the generated future state machine cannot hold a reference pointing to another field within the same future state machine.
    3.  *Design Consequence*: Forbidding self-referential futures natively eliminates the requirement for complex, user-facing `Pin` types in Mellis.

### Hole 4: `Const × Comptime` Boundary (Semantics vs. Representation)
*   **The Distinction**:
    - **Semantic Level**:
        - `const`: A pure compile-time constant evaluated during compilation as a static expression.
        - `comptime { expr }`: An arbitrary Mellis execution block evaluated in the comptime VM during compilation, returning a computed value.
    - **Representation Level (Decoupled)**:
        - How the resulting value is materialized (inlined immediate literal, constant pool entry, or `.rodata` global) is a compiler optimization and target ABI decision, **not** an inherent language semantic rule.

### Finding 5: Associated Type Equality & The Monomorphization Barrier (Rule C)
*   **Critical Soundness Impact**: The `Try` protocol fundamentally depends on associated types:
    $$\text{Try} \longrightarrow \text{Self::Output}, \text{Self::Residual} \longrightarrow \text{FromResidual}$$
*   **Frozen Normative Invariant (Rule C)**: **Associated type projections may remain symbolic during generic analysis, but no unresolved projection may cross the monomorphization boundary into MVIR/backend.**
*   **Lifecycle**: Projections normalize eagerly when concrete types are known. At the monomorphization boundary, zero symbolic projections, generic parameters, or inference variables are allowed.

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
│   ├── Const × Comptime (Evaluation model formalization)
│   └── Raw Pointer Safety & Unsafe Boundary (Dereference rules)
│
└── P3 (Polymorphism & Coercions)
    └── Unsize / Dyn Trait Coercion Formalization
```

