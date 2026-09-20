# Luna Lifetime Model Consolidation (REGION-DESIGN-01)

## 1. Motivation

Luna requires a formal lifetime and region model to decouple language semantics from the compiler's borrow-checker implementation details. A formal model establishes a stable, predictable language contract for developers, ensures consistency across compiler phases, and provides a clear boundary for what the language guarantees versus what the compiler happens to infer. Most importantly, it shifts Luna away from "borrowck implementation behaviors" toward a robust semantic foundation.

## 2. Design Philosophy

Luna's lifetime model is built upon five core invariants and one overarching soundness principle. These distinguish Luna's semantic philosophy from other languages like Rust.

### Core Invariants

- **Place Identity ≠ Loan Liveness**: The memory location (Place) is independent of its current borrowing state (Loan).
- **Value Provenance ≠ Type Syntax**: The presence of pointer syntax in a type does not inherently confer data provenance. For example, a function pointer `fn(*i32)` is a code pointer, not a data provenance carrier.
- **Compiler Capability ≠ Language Semantics**: The fact that the compiler *can* infer a relationship does not mean it *should* do so implicitly. The language contract must remain explicit.
- **Static Place ≠ Dynamic Lifetime Instance**: A statically declared Place (e.g., a variable inside a loop) creates distinct dynamic lifetime instances at runtime (one per iteration).

### Semantic First Principle

**Lifetime Relation ≠ Lifetime Annotation.** The true semantic meaning is the constraint (e.g., $R_1 \succeq R_2$), not the syntax used to express it (e.g., `'a`). Annotation is merely syntax sugar over semantic relations. Luna prioritizes "semantics first, syntax second."

### Lifetime Soundness Principle

A formal definition for borrow validity:
```text
Valid(reference) = 
    Provenance exists AND 
    Region constraints satisfied AND 
    Loan capability is active AND
    Capability not violated
```
A reference is valid only if its provenance exists, the active loan is maintained, the capability is respected, and every region constraint is satisfied.

**Note:** Validity is a property of reference *usage*, not a property of the reference value alone. The same reference value may be valid at one point in the control flow and invalid after crossing a region boundary.

## 3. Terminology

To avoid ambiguity, Luna standardizes the following terms:

- **Place**: A static, syntactic expression representing a memory location (e.g., `x`, `s.field`).
- **Storage**: The actual physical memory allocation corresponding to a Place during execution.
- **Provenance**: The source of a borrowed value. It tracks which Place(s) a reference points to and governs aliasing.
- **Loan**: A temporary capability (shared or mutable) granted over a Place.
- **Region**: A compiler-level abstraction representing a validity domain in which a provenance relationship may remain usable. **Crucially, a Region does not own memory and does not represent storage duration.**
- **Lifetime**: A set of constraints over Regions that determine whether a reference usage is valid.
- **Relation**: The mathematical relationship between two Regions (e.g., outlives, intersects).
- **Capability**: The permissions (e.g., `rw`) a value holds over its target.

## 4. Layer Architecture

Luna's lifetime model is cleanly separated into four architectural layers. Note the conceptual dependency direction:

```text
                User Contract
                      |
              life_from / requires
                      |
              Lifetime Relation Graph
                      |
        +-------------+-------------+
        |                           |
   Provenance                  Capability
        |                           |
        +-------------+-------------+
                      |
                   Loan
                      |
                   Place
                      |
                  Storage
```

Thus, the structural foundation is:
1. **Layer 1: Place / Storage Model** (Storage implements Place identity)
2. **Layer 2: Provenance Model** (Place defines identity; Provenance links references to Places)
3. **Layer 3: Capability & Region Model** (The active Loan capability and the Region constraints operate in parallel to form Reference Validity)
4. **Layer 4: Type & API Contract Model** (Lifetime is exposed through Type contracts)

## 5. Lifetime Relation Model

A fundamental departure from older paradigms is that **a lifetime in Luna is not an absolute duration of time or an object**. It is a **relation**. When we say a reference has a lifetime, we mean its Region satisfies certain constraints relative to the Region of its Provenance.

The three basic relations are:

1. **Outlives ($R_1 \succeq R_2$)**: $R_1$ outlives $R_2$. It signifies that $R_1$ has a longer or equal validity domain. (Note: $\succeq$ denotes longer validity, not necessarily a temporal ordering).
2. **Intersection**: When returning a reference from multiple inputs (e.g., `fn max(a: &i32, b: &i32) -> &i32`), the return region is bound by an intersection. The compiler does not synthesize a new region $Region(R_a \cap R_b)$; instead, it generates multiple constraints: $R_{ret} \preceq R_a$ and $R_{ret} \preceq R_b$.
3. **Substitution / Binding**: When a region is bound to a data structure (e.g., `Holder[R_x]`), it acts as a semantic substitution. This internal semantic binding does not necessarily map to explicit generic syntax for the user.

## 6. Lifetime Relation Graph

The relational model naturally forms a **Lifetime Relation Graph**. 
A lifetime graph consists of:
- **Region Nodes** (e.g., Function Region, Input Region, Return Region)
- **Constraint Edges** (including outlives relations, equality, binding, and intersection constraints)

The compiler's task during borrow checking is to Generate constraints, Solve the graph, and Reject impossible relations.

**Example 1: Single Input**
```rust
fn identity(x: &i32) -> &i32
```
Graph: 
```text
InputRegion(x)
      |
      | outlives
      v
 ReturnRegion
```
Constraint: `InputRegion(x) succeq ReturnRegion`

**Example 2: Multiple Inputs (Intersection)**
```rust
fn choose(a: &i32, b: &i32) -> &i32
```
Graph:
```text
     R_a
      |
      | outlives
      v
    R_ret
      ^
      | outlives
      |
     R_b
```
Constraints:
`R_a succeq R_ret`
`R_b succeq R_ret`

## 7. Current Borrow Model

Luna's implementation currently has a robust foundation handling lower-level lifecycle management based on control flow:
- **Borrowck**: Actively tracks active loans and detects aliasing violations.
- **NLL (Non-Lexical Lifetimes)**: Allows loans to expire early based on their last use.
- **CFG Integration & Escape Checking**: Accurate control-flow propagation for borrowing states.

## 8. Region Kinds

Before building the Relation Graph, Luna categorizes regions into specific kinds:

- **Lexical Region**: Function or block scopes.
- **Dynamic Region**: Loop iterations.
- **External Region**: FFI boundaries and cross-module contracts.
- **Program Region**: Represents the maximal validity scope (the entire program execution). Replacing the confusing "Static Region", this clarifies that it represents a scope, not necessarily static memory.

## 9. Lifetime Boundary Model

Luna introduces strict boundaries where regions are transformed, terminated, or enforced:

### Internal Region Boundaries
- **Function Return**: Local regions are destroyed; escaping regions are checked against API contracts.
- **Loop Back-edge**: The previous iteration's dynamic region is destroyed, and a new one is recreated.
- **Closure Escape**: Captured regions must outlive the closure's region.

### External Region Boundaries
- **FFI Call**: A trust boundary where implicit semantic leakage is forbidden.
- **Module Artifact (`.llib`)**: Binary boundary requiring strict adherence to canonical lifetime contracts.

## 10. User Contract Syntax (Frozen)

Luna adopts a formal explicit contract syntax that preserves inference ergonomics while enforcing explicit semantic boundaries. Luna does not use Rust-style named lifetime generic parameters (`<'a>`), nor does it expose region variables directly. 

The syntax primitives form the complete surface area of Luna's lifetime contract model:

### 10.1 Provenance Binding: `life_from`
Strictly used at boundaries (function return, extern return, trait method return) to indicate that a returned reference acquires its lifetime constraints from one or more sources. It is **not** used for internal stored references (e.g., struct fields).

**Single Source:**
```rust
fn identity(x: &i32) -> &i32
    life_from(x)
```
*Semantic meaning:* $R_{return} \preceq R_x$

**Multiple Sources (Intersection Constraints):**
```rust
fn choose(a: &i32, b: &i32) -> &i32
    life_from(a, b)
```
*Semantic meaning:* $R_{return} \preceq R_a$ AND $R_{return} \preceq R_b$.

### 10.2 Lifetime Constraint: `requires`
Used to express general outlives constraints between regions.

```rust
fn process(owner: &Owner, value: &Value)
requires
    life(owner) >= life(value)
```
*Semantic meaning:* $R_{owner} \succeq R_{value}$ (owner outlives value).

*(Note: Equality constraints are simply expressed by requiring bidirectional outlives, e.g., `requires life(a) >= life(b) && life(b) >= life(a)` if absolutely necessary).*

### 10.3 Lifetime Query and Projection: `life`
Used to query the region constraint of a reference, or when projecting a lifetime from a composite type.

```rust
struct User {
    name: &str
}

fn get_name(u: &User) -> &str
    life_from(u.name)
```

### 10.4 Special Lifetime Targets
Luna provides special keywords to query the lifetime of context-specific regions:
- `life(self)`: The region constraint of the current object instance.
- `life(return)`: The region constraint of the returned value.

```rust
struct Holder {
    value: &i32
}
requires
    life(value) >= life(self)
```

### 10.5 FFI Contract
There is **no inference** across FFI boundaries. Explicit contracts are mandatory for safe references.

```rust
extern fn get_name(user: *User) -> &str
requires
    life(return) <= life(user)
```

### 10.6 Closure Boundaries
Closure constraints are implicit in 99% of normal usage. The compiler automatically synthesizes $R_{captured} \succeq R_{closure}$ (captured provenance must outlive the closure).
If a closure escapes its valid region, the compiler will reject it (e.g., `E3005: closure lifetime exceeds captured provenance`).


## 11. Lifetime Elision (LLE)

Lifetime Elision reduces boilerplate but strictly follows the semantic design, never dictating it. 

**Frozen Elision Rules:**
- **Case 1: Single input reference**
  `fn foo(x: &T) -> &T` is automatically elided to `life_from(x)`.
- **Case 2: Multiple input references**
  `fn foo(a: &T, b: &T) -> &T` has **no elision**. The compiler will reject this unless explicitly annotated (e.g., `life_from(a, b)`).
- **Case 3: No input references**
  `fn foo() -> &T` is statically rejected as an impossible provenance.

## 12. Compiler Responsibilities

**Allowed Inference:**
- The compiler is encouraged to perform flow-sensitive tracking, NLL, and intra-procedural borrow analysis.

**Forbidden Inference:**
- The compiler MUST NOT implicitly guess the lifetime of references crossing FFI boundaries.
- The compiler MUST NOT silently synthesize unbounded (`'static`-like) lifetimes for raw pointer dereferences.
- The compiler MUST NOT infer lifetime extension from storage reuse (e.g., reusing stack slots across loop iterations does not imply the same lifetime instance).

## 13. Lifetime Model Non-goals

The Luna Lifetime Model explicitly **DOES NOT** aim to:
- Replicate Rust's named lifetime syntax (`'a`).
- Expose all internal compiler regions to the end-user.
- Attempt to prove the safety of arbitrary `unsafe` memory manipulation.
- Automatically infer FFI lifetimes without an explicit user-provided contract.

## 14. Future Implementation Roadmap

This formal design document paves the way for the implementation phases:

- **REGION-SPEC-01: Lifetime Relation Formal Specification** (Completed & Frozen ✅)
  - **Deliverables:** Mathematical definition of Region algebra, constraint solving rules, relation propagation, error failure model, and normative appendices for Region ordering, Reference validity typing judgment, and solver guarantees (`docs/spec/lifetime-formalism.md`).
- **REGION-01: Formal Region Graph Engine** (Completed & Frozen ✅)
  - **REGION-01A: Region IR Foundation** (Completed & Frozen ✅)
    - `RegionId`, `RegionKind` (Program, Lexical, Iteration, External, Contract, Inference), `RegionGraph`, canonical Outlives constraints, 3-axis metadata origin.
  - **REGION-01B: Constraint Generation** (Completed & Frozen ✅)
    - Dual generators: `ContractConstraintGenerator` (lowers `life_from`, `requires`, struct contracts, LLE elision, artifact parity) and `BodyConstraintGenerator` (lowers `&`/`&rw` borrows, closure captures, structural scope/loop nesting, negative raw pointer rule).
  - **REGION-01C: Symbolic Region Relation Solver** (Completed & Frozen ✅)
    - Derived non-mutating query solver (`RegionSolution`). Computes SCC/equality normalization, condensation DAG, transitive relation entailment, axiomatic Program top bound, static bound validation, and deterministic proof explanation paths preserving 3-axis metadata.
  - **REGION-01D: CFG Region Realization & Borrow Integration** (Completed & Frozen ✅)
    - Dual-fact realization: **Point Facts** ($p \in R$) and **Boundary Facts** (CFG edge $e$ terminates dynamic region instance $R$).
    - 4-phase completion:
      - `01D-A`: CFG Point & Boundary Facts (`ProgramPoint`, `RegionUseRequirement`, `RegionBoundaryExit`, fixed bounds) — **FROZEN ✅**
      - `01D-B`: Region Point Realization (`RegionRealization`, upward propagation, bounds validation, dynamic instance exit validation) — **FROZEN ✅**
      - `01D-C`: Borrowck Bridge / Shadow Validation (shadow mode comparing verdicts with zero divergence across maturity suites) — **FROZEN ✅**
      - `01D-D`: Authority Cutover (Region engine became sole lifetime authority, borrowck delegated strictly to loan/capability/move legality, verified 10/10 in `CUTOVER-01..10`) — **FROZEN ✅**

### Normative Authority Invariant
> *"Region authority determines whether a reference lifetime is valid. Borrow authority determines whether an otherwise lifetime-valid operation is legal under provenance, loan, capability, and move rules. Neither subsystem may reconstruct or override the other's verdict."*

### Canonical Lifetime Pipeline
```text
Language Semantics
        ↓
Lifetime Formalism
        ↓
Region Constraint Generation
        ↓
Symbolic Region Solving
        ↓
CFG Region Realization
        ↓
Borrow Integration
```
*Rule:* If a lifetime case cannot be expressed within the formal Region system, it is a design gap requiring semantic review, never an ad-hoc heuristic patch inside `BorrowAnalyzer`.

### Canonical Architecture & Roadmap Status

| Milestone | Scope / Responsibility | Canonical Status |
|---|---|---|
| **SEM-MATURITY-01** | Compiler semantic test suite & maturity stabilization | **COMPLETE & FROZEN ✅** |
| **REGION-DESIGN-01** | Architectural design and separation of concerns | **FROZEN ✅** |
| **REGION-SPEC-01** | Formal mathematical and lattice specification | **FROZEN ✅** |
| **REGION-01** | Formal Region Graph Engine | **COMPLETE & FROZEN ✅** |
| ├─ **REGION-01A** | Region IR, Graph, Regions, Points, Relations | **FROZEN ✅** |
| ├─ **REGION-01B** | Semantic Constraint Generation (Contract + Body) | **FROZEN ✅** |
| ├─ **REGION-01C** | Symbolic Region Relation Solver | **FROZEN ✅** |
| └─ **REGION-01D** | CFG Realization + Borrow Authority Cutover | **FROZEN ✅** |
| *(Old REGION-02)* | *Borrowck Integration* | *Subsumed by REGION-01D-C & 01D-D* |
| **Memory-Safety Authority Migration** | End-to-end delegation of all lifetime checks to Region Engine | **PARTIAL until REGION-02A cutover** |

### Post-Freeze Maintenance
- **REGION-HARDENING: Canonical Loop Tree Hierarchy** *(Non-semantic refactor / robustness item)*
  - Replace `min_by_key(|nl| nl.blocks.len())` with explicit loop nesting metadata: `LoopInfo { id, parent: Option<LoopId>, depth: usize, blocks }` and `max_by_key(|nl| nl.depth)`.
  - **Invariants:**
    - *"Loop nesting identity is semantic/compiler-structural metadata. Block-set size is never a lifetime semantic."*
    - Hardening must preserve 100% of observable lifetime behavior and pass complete maturity + Region suites.

### Next Semantic Milestone
- **REGION-02: User Contract Pipeline Alignment & Call-Site Authority Completion**
  - **REGION-02A: Call-Site Outlives Authority Cutover** *(Priority 1)*
    - Eliminate `BorrowAnalyzer::root_outlives` and the lexical heuristic `v_long.0 <= v_short.0`.
    - Realize pipeline:
      $$\text{CanonicalLifetimeContract} \longrightarrow \text{LifetimeSubject} \longrightarrow \text{Argument RegionIds} \longrightarrow \text{RegionGraph / RegionSolution} \longrightarrow \text{Outlives Verdict} \longrightarrow \text{Diagnostic E2016}$$
    - **Invariant:** *"Reordering unrelated ValueIds MUST NOT change outlives contract satisfaction."*
  - **REGION-02B: Canonical Surface Syntax Alignment**
    - Canonical syntax: `requires life(a) >= life(b)`.
    - Legacy compatibility syntax: `where outlives(a, b)` (both lower to identical semantic relation $R_a \succeq R_b$).
    - `life(return)`: `requires life(source) >= life(return)` (parser syntax node -> resolver `LifetimeSubject::Return`).
  - **REGION-02C: Resolved Contract IR / Struct Contract Completion**
    - Complete struct contracts in parser and AST (`Decl::Struct`): `struct Holder { value: &T } requires life(value) >= life(self)`.
    - Pipeline: $\text{Parser AST} \longrightarrow \text{Resolver} \longrightarrow \text{ResolvedLifetimeContract} \longrightarrow \text{CanonicalLifetimeContract}$.
    - Semantic boundary: *"source spelling $\ne$ semantic identity"* (field projections resolve to symbolic identities, not strings).
  - **REGION-02D: End-to-End Contract Parity & Legacy Compatibility**
    - Cross-verify `where outlives` and `requires life() >= life()`.
    - Verify identical `CanonicalLifetimeContract` and call-site Region verdicts across source and `.llib` providers.
    - Guarantee struct contracts survive `.llib` metadata serialization.

