---
name: luna-semantic-compliance
description: Mandatory semantic compliance protocol for Luna language. Enforces writing canonical Luna syntax and genuine Luna semantic contracts instead of Rust/C++/LLVM assumptions.
---

# Luna Semantic Compliance Protocol

## Purpose

Ensure every Luna source artifact, standard-library implementation, ABI-facing Luna code, compiler-generated Luna code, test fixture, and semantic integration uses the actual Luna language semantics rather than assumptions borrowed from Rust, C++, TypeScript, LLVM, or another language.

This skill is **mandatory** whenever the agent:
- writes or modifies `.ln` / `.ms` source;
- writes Core / Alloc / Stdlib code;
- writes compiler-generated Luna source;
- designs ABI-facing Luna APIs;
- writes semantic integration tests;
- migrates legacy Mellis code;
- adds examples to language documentation;
- reviews code that claims to use a Luna language feature.

The compiler implementation and frozen language specifications are authoritative. Agent memory, intuition, external language conventions, and generated examples are not.

---

# 1. Core Principle

Before writing Luna code, determine:
1. What is the exact Luna semantic contract?
2. Does Luna actually expose this feature in source syntax?
3. What is the canonical Luna syntax?
4. What semantic phase enforces it?
5. Is the feature frozen?
6. Is there a simpler Luna-native way to express the same intent?

Never substitute a familiar syntax or semantic model from another language.

Examples of forbidden assumptions:
- Rust lifetime syntax `'a`, `<'a>`, `T: 'a`
- Rust `let`, `mut`, `use`, `mod`
- Rust turbofish `::<>`
- C/C++ ownership assumptions
- LLVM representation treated as Luna type semantics
- Backend representation used as source-language semantics

---

# 2. Semantic SSOT Hierarchy

When determining Luna behavior, consult sources in this order:

## Level 1 — Frozen Language Specification
Normative language specification and frozen design documents.
- Surface syntax specification
- Semantic core specification
- Lifetime relation model specification (`life_from`, `where outlives`)
- Module/import specification (Provider vs Namespace)
- Runtime ABI specification
- MVIR memory lifecycle specification
- Diagnostic specification

If implementation behavior conflicts with a frozen specification, the conflict must be reported rather than silently redefining the language.

## Level 2 — AST / Parser / Semantic Implementation
Use these to verify what is actually implemented:
- `luna-lexer`
- `luna-parser`
- `luna-ast`
- `luna-semantic`

## Level 3 — Borrowck / Mono / MVIR
Use these to understand enforcement and propagation:
- `luna-borrowck`
- `luna-mvir`
- `luna-driver`

## Level 4 — Stdlib / Core Examples
Use Core / Alloc / Stdlib as examples of correct Luna usage, but never treat existing library code as authoritative if it conflicts with the language specification.

## Level 5 — Tests
Tests are evidence of implementation behavior. Tests do NOT redefine the semantic specification. A green test does not prove semantic correctness if the test encodes an incorrect contract.

---

# 3. Mandatory Semantic Preflight

Before implementing a non-trivial Luna API or test fixture, produce an internal preflight:

```text
Feature:
Luna syntax:
Semantic rule:
Enforcing phase:
ABI/runtime implications:
Existing canonical example:
Potential foreign-language assumption:
```

If any field is unknown, inspect the relevant SSOT before coding.

---

# 4. Luna Syntax Compliance

The agent MUST use canonical Luna syntax.

Important frozen examples:
```rust
dec x: i32 = 1;
const y: i32 = 2;
```

Do NOT use `let` or `mut`.

Mutation/access uses `rw`:
```rust
dec rw x: i32 = 1;
&rw x
```

References:
```rust
&T
&rw T
```

Raw Pointers:
```rust
*T
*rw T
```

Imports:
```rust
import <core>;
import <alloc>;
import "local_module";
```

Do NOT invent extension-bearing imports such as `import "core.ln";` or `import <core.llib>;`.

Modules (inline namespaces):
```rust
module math {
    export struct Point { x: f64, y: f64 }
}
```

Do NOT invent file-style module declarations (`module math;`).

Generic arguments use Luna syntax:
```rust
Type<T>
foo<T>(...)
```

Do not introduce Rust turbofish syntax (`foo::<T>()`).

Match arms use `->`:
```rust
match val {
    Option::Some(x) -> x,
    Option::None -> 0,
}
```
`=>` is strictly used as macro transcriber separator, NOT match arms.

Await is strictly postfix:
```rust
expression.await
```
No prefix `await expression`.

---

# 4.1 Struct Field Visibility Invariants (VIS-STRUCT)

Struct field visibility is independent from struct type visibility. The semantic contracts are strictly frozen:

- **VIS-STRUCT-1**: Field visibility is independent from struct visibility.
- **VIS-STRUCT-2**: A private struct cannot export fields (`Struct field cannot be declared 'export' in a private struct`).
- **VIS-STRUCT-3**: Private field access is permitted only from the field's defining module scope and allowed descendants (ancestor rule).
- **VIS-STRUCT-4**: Struct type visibility is checked before field visibility.
- **VIS-STRUCT-5**: External construction requires access to every required/private field. Struct literals cannot be constructed from an external scope if the struct contains any private fields.
- **VIS-STRUCT-6**: Private fields cannot be read, written, borrowed, projected, or destructured from an inaccessible scope. All 6 access paths are enforced:
  1. Field read: `u.field`
  2. Field write / assignment: `u.field = val`
  3. Field borrow: `&u.field`, `&rw u.field`
  4. Struct literal construction: `User { name: ..., password: ... }`
  5. Destructuring / pattern match: `match u { User { password, .. } -> { ... } }`
  6. Nested field projection: `acc.user.password`
- **VIS-STRUCT-7**: Source `.ln` and `.llib` preserve identical field visibility semantics across binary boundaries.

Key Regression Matrix:
- `export struct + private field`: Valid declaration; private fields accessible within defining module/descendants, rejected externally.
- `struct + export field`: Rejected at resolver phase (`VIS-STRUCT-2`).
- `nested module access`: Allowed if accessing scope is a descendant of the defining module scope; rejected for sibling or parent modules without permission.
- `external provider`: Private fields strictly rejected across provider boundaries.

---

# 4.2 Core Pointer & Memory Invariants (PTR-MEM)

Provider `<core>` exports low-level raw pointer and memory manipulation primitives under two distinct module namespaces: `module ptr` and `module mem`.
Consumer usage strictly follows Rule 7 (Provider ≠ Namespace):
`import <core>;` brings provider `<core>` into the compilation context; symbols are accessed via `ptr::*` and `mem::*`. `core::ptr::*` is invalid and rejected.

The 5 semantic contracts are strictly frozen:

- **PTR-MEM-1 (No Automatic Lifetime Inference)**:
  Raw pointer operations are explicit `unsafe` escape hatches. Calling `ptr::read` or dereferencing a raw pointer does NOT synthesize or infer a safe lifetime relation (`life_from`). Returned values are owned; raw pointers never masquerade as borrowed references.
- **PTR-MEM-2 (Semantic Pointer Arithmetic & Allocation Provenance)**:
  Pointer arithmetic (`ptr::add`, `ptr::add_mut`, `ptr::offset`, `ptr::offset_mut`, `ptr::diff`) is defined at the language semantic level over $T$-sized elements (not defined as source-level integer casts).
  - `ptr::diff(a, b)` computes signed element distance $((a - b) / \text{sizeof}(T))$.
  - **Allocation/Provenance Constraint**: Both `a` and `b` MUST point within the same allocated object or one-past-the-end. Cross-allocation or cross-provenance pointer difference is undefined behavior at the semantic level.
- **PTR-MEM-3 (MVIR Memory Lifecycle / Move / Drop Compliance)**:
  Raw pointer writes and reads interact directly with MVIR lifecycle rules:
  - `ptr::read<T>(src: *T) -> T`: Semantically executes a `MoveOut` of $T$ out of raw memory. Ownership of $T$ transfers to the caller, which assumes standard drop obligations. If $T$ is non-`Copy`, the source memory slot is conceptually left uninitialized. Subsequent reads without re-initialization yield undefined behavior / double-free hazards.
  - `ptr::write<T>(dst: *rw T, val: T)`: Semantically executes an `Initialize` on destination raw memory without invoking `DropInPlace` on any previous value in that cell. Overwriting an already-initialized $T$ that owns resources causes those resources to leak unless the caller explicitly destructs them first.
- **PTR-MEM-4 (Explicit Count Units)**:
  Units for count parameters are explicitly bifurcated:
  - `ptr::*` functions operate strictly on **typed element counts** ($N$ elements of type $T$).
  - `mem::*` block functions operate strictly on **raw byte counts** (`bytes: u64`).
  - Typed block copy `ptr::copy<T>(src: *T, dst: *rw T, count: u64)` operates on **element counts** ($N \times \text{sizeof}(T)$ bytes).
- **PTR-MEM-5 (Source & .llib Parity)**:
  Module definitions, symbol visibility, and parameter signatures in `core.ln` and precompiled `core.llib` are strictly identical across binary boundaries.

---

# 5. Lifetime / Borrow Semantic Compliance

Luna does NOT use Rust-style named lifetime parameters.

Never invent:
- `'a`, `'b`
- `<'a>`
- `T: 'a`

Luna uses relation-based lifetime syntax:

```rust
// Return provenance from single parameter
fn first(a: &i32) -> &i32 life_from(a) {
    return a;
}

// Multiple provenance sources (union)
fn pick(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    return a;
}

// Outlives constraint (longer >= shorter)
fn merge(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)
{
    return a;
}
```

Semantic interpretation:
- `life_from(x)`: Return provenance originates from `x`.
- `life_from(a | b)`: Return provenance may originate from `a` or `b`.
- `outlives(longer, shorter)`: `lifetime(longer) >= lifetime(shorter)`.

When implementing lifetime-aware stdlib APIs, the agent MUST use these relations. Do not rely on implicit lifetime assumptions merely because an API "looks like" a Rust equivalent.

---

# 6. Provenance Is Semantic Contract, Not Borrowck Implementation Detail

The agent must distinguish:
- **Semantic contract**: `life_from(...)`, `where outlives(...)`, `CanonicalLifetimeContract`.
- **Derived implementation analysis**: `ReturnEffect`, `CallEffectSummary`, `Loan`, `direct_provenance`, `carried_provenance`.

Never make an ABI or `.llib` public contract depend directly on Borrowck internal implementation structures. `.llib` preserves canonical semantic lifetime contracts (`CanonicalLifetimeContract`), not internal Borrowck objects.

---

# 7. Reference-Returning API Rule

Any Luna API returning a reference MUST answer:
**Where does the returned reference originate from?**

- If it originates from an input/receiver, express it explicitly:
  ```rust
  fn get(&self, index: usize) -> Option<&T> life_from(self)
  ```
- If it has multiple possible sources:
  ```rust
  life_from(a | b)
  ```
- If a required lifetime ordering exists:
  ```rust
  where outlives(longer, shorter)
  ```

Do NOT silently return borrowed values without checking whether the language contract requires an explicit relation.

---

# 8. Aggregate / Generic Reference Rule

Before implementing:
- `Option<&T>`
- `Vec<&T>`
- `struct Span { start: &T, end: &T }`

verify how Luna represents and propagates provenance:
1. Never assume Rust-style generic lifetime parameters.
2. Verify: `Variant`, `Extract`, `Store`, `Load`, `FieldPtr`, `carried_provenance`, aggregate return contracts.
3. Current Luna v1 treats multiple fields as one whole-value borrow domain. Disjoint field borrowing is deferred to Borrowck v2. Stdlib code must respect that behavior rather than assuming disjoint field borrowing.
4. `Vec<&T>` guarantees soundness through borrowed container APIs (`&self -> &T life_from(self)`), but does not have element-level heap provenance tracking in v1.
5. Aggregate functions returning local references must declare `life_from(...)` to trigger full escape checking.

---

# 9. ABI / .llib Preservation Rule

For every semantic feature exposed through `.llib`:
$$\text{Source semantics} \longrightarrow \text{Canonical semantic representation} \longrightarrow \text{.llib metadata} \longrightarrow \text{Consumer reconstruction} \longrightarrow \text{Borrowck / TypeChecker}$$

The source and binary path must preserve identical semantics.

For lifetime-aware signatures, verify:
- `life_from(...)`
- `where outlives(...)`
- return reference type
- 0-indexed parameter positions
- trait method lifetime contracts

Never:
- serialize session-local IDs;
- serialize AST parameter names as semantic identity;
- serialize Borrowck implementation structs as the ABI contract;
- silently drop semantic lifetime relations.

---

# 10. Stdlib Design Rule

When adding a Core / Alloc / Stdlib API, first ask:
**"Which Luna semantic feature should this API exercise?"**

Examples:
- `Option::as_ref` → borrowed projection → `life_from(self)`
- `Vec::get` → borrowed element → `life_from(self)`
- `Vec::iter` → iterator borrows owner → `life_from(self)`
- `Iterator::next_ref` → propagated iterator provenance

The goal is NOT to imitate Rust's API surface. The goal is to create an API that is correct under Luna's own semantics.

---

# 11. Feature Utilization Audit

When writing standard-library code, check for semantic under-utilization.

Flag this anti-pattern:
*Compiler supports feature X, but stdlib avoids X entirely even where it is semantically required.*

Examples:
- Lifetime relation exists, but every API returns owned values.
- Borrow checking exists, but public collection APIs bypass it through raw pointers.
- Module system exists, but library code depends on synthetic provider namespaces.
- `rw` exists, but mutable APIs are expressed through raw-pointer mutation instead.
- Generics exist, but APIs unnecessarily monomorphize concrete duplicate implementations.

**Rule:** Use the feature when semantically appropriate. Do not bypass it just because another language does so.

---

# 12. Raw Pointer / Unsafe Escape Hatch Rule

Raw pointers are not a replacement for ordinary Luna references.

Before introducing `*T` or `*rw T` or `unsafe` blocks, answer:
1. Why is a normal Luna reference insufficient?
2. Which safety invariant replaces Borrowck here?
3. Where is invalidation controlled?
4. Can the API accidentally expose dangling state?

A raw-pointer-based implementation must never silently weaken the safety contract of a safe public API.

---

# 13. Trait / Impl Compliance

When implementing a trait method:
1. Resolve the trait method's canonical semantic contract.
2. Preserve its lifetime relation.
3. Preserve its `rw` semantics.
4. Preserve generic/type constraints.
5. Check implementation contract against trait contract:
   - Outlives Preconditions: $\text{Pre}_{\text{trait}} \implies \text{Pre}_{\text{impl}}$ (Contravariance)
   - Return Provenance: $\text{Prov}_{\text{impl}} \subseteq \text{Prov}_{\text{trait}}$ (Provenance Contraction)
6. Use official Luna terminology: **"Provenance Contraction"** and **"Sound Subcontracting"**. Do NOT refer to it as Rust lifetime covariance.

---

# 14. ABI and Runtime Boundary

Never derive Luna semantic types from backend representation:
- Luna `char` $\ne$ arbitrary LLVM integer representation
- Luna `str` $\ne$ backend pointer type
- Luna `void` $\ne$ Rust unit `()`
- Luna reference $\ne$ raw pointer merely because backend lowers it to one

The compiler frontend/middle-end owns semantic meaning; backend lowering is a later representation decision.

---

# 15. Implementation Discipline

When the agent discovers that a needed Luna feature is missing:

**DO:**
1. Stop using an invented workaround.
2. Identify the semantic gap.
3. Classify:
   - implementation bug
   - missing semantic feature
   - missing stdlib API
   - intentional limitation
4. Update the appropriate phase/spec.

**DO NOT:**
- silently approximate semantics;
- use a foreign-language workaround;
- weaken Borrowck;
- erase lifetime/provenance metadata;
- introduce compatibility syntax without specification;
- add compiler magic solely to make stdlib code compile.

---

# 16. Mandatory Verification for ABI / Stdlib Changes

For a semantic-sensitive API, verify at least:
1. Source `.ln` compilation
2. Canonical `.llib` compilation/loading
3. TypeChecker behavior
4. Borrowck behavior
5. Negative misuse case
6. Source vs `.llib` parity
7. Full workspace regression

For lifetime-sensitive APIs, additionally test:
- returned borrow remains valid
- owner mutation while borrow is alive is rejected
- owner mutation after borrow death is accepted
- local escape is rejected (`E3005`)
- provenance mismatch is rejected (`E2016`)
- multi-source provenance behaves correctly

---

# 17. Required Agent Report Format

After implementing a semantic-sensitive change, report:

## Semantic Contract
What Luna rule is being exercised?

## Luna Syntax
What exact Luna syntax is used?

## Semantic Enforcement
Which compiler phase enforces it?

## Implementation
Which files changed and why?

## Feature Usage
Which Luna-specific features are intentionally used?

## Foreign Assumptions Avoided
Which Rust/C++/LLVM assumptions were explicitly avoided?

## ABI / .llib
How is the semantic contract preserved across binary boundaries?

## Negative Tests
What invalid Luna program must fail?

## Source / Binary Parity
Do `.ln` and `.llib` behave identically?

## Verification
List targeted tests and workspace status.

## Remaining Limitations
Separate:
- soundness limitation
- precision limitation
- unsupported feature
- unrelated debt

---

# 18. Final Invariant

Before committing any Luna source implementation, ask:
> *"Am I writing this because Luna specifies it, or because I am unconsciously writing another language?"*

If the answer is the second one, stop and re-check the Luna semantic SSOT.

The objective is not Rust-like code written with Luna syntax.  
The objective is code that is **semantically Luna**.
