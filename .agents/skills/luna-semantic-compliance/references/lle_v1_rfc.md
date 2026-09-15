# RFC: Luna Lifetime Elision v1 (LLE-v1)

**Status**: FROZEN (approved 2026-09-13)
**Depends on**: Lifetime Relation Model v1.0 (FROZEN)
**Architectural position**: Surface ergonomic layer — does NOT modify the underlying semantic model.

---

## Motivation

Luna's lifetime system (`life_from`, `where outlives`) is explicit-by-design and semantically correct. However, for the majority of stdlib APIs — single-provenance, single-receiver patterns — the annotation is mechanically derivable and syntactically noisy.

LLE-v1 introduces a **compiler elaboration phase** that infers lifetime contracts when unambiguous, allowing programmers to omit `life_from(...)` and `where outlives(...)` in trivial cases while preserving full explicit control for complex signatures.

```
           Surface Syntax
                 ↓
          ┌──────────────────┐
          │     explicit     │     omitted
          └──────────────────┘
                 ↓
          Lifetime Elaboration (LLE)
                 ↓
          CanonicalLifetimeContract
                 ↓
     ┌───────────┼────────────┐
     │           │            │
  Borrowck     MVIR        .llib
```

> [!IMPORTANT]
> LLE only makes source shorter. It does NOT create a second semantic model. The `CanonicalLifetimeContract` produced by LLE is identical whether the programmer wrote the annotation explicitly or the compiler inferred it.

---

## Inference Rules

### LLE-1: Single Legal Provenance Candidate → Infer `life_from`

When a function returns a reference type and the parameter list contains **exactly one legal provenance candidate**, the compiler infers `life_from(candidate)`.

**Definition — Legal Provenance Candidate**: A parameter (including `self`) that can legally originate the returned reference. This is determined by semantic elaboration:

```
parameters
   ↓
collect lifetime-bearing / provenance candidates
   ↓
filter candidates that can legally originate returned reference
   ↓
if exactly 1 → infer
if >1         → ambiguity → require explicit life_from(...)
if 0          → reject (see LLE-4)
```

> [!IMPORTANT]
> The key unit is **legal provenance candidate**, not merely "parameter with reference type". A parameter bearing `&T` is not automatically a provenance candidate for the return — semantic elaboration must confirm the return reference can legally originate from that parameter.

**Example — inference applies:**
```rust
fn get(self: &Self, index: u64) -> Option<&T>
// legal provenance candidates = [self]  (index: u64 has no provenance)
// inferred: life_from(self)
```

**Example — ambiguity, explicit required:**
```rust
fn pick(a: &A, b: &A) -> &A
// legal provenance candidates = [a, b]
// → ambiguity → programmer must write life_from(a) or life_from(a | b)
```

**Example — nested provenance:**
```rust
fn foo(a: &T, b: Wrapper<&U>) -> &T
// Even though b carries a reference inside aggregate,
// legal provenance candidates for return &T may only be [a]
// (if &U inside Wrapper cannot originate &T)
// → infer life_from(a)
```

---

### LLE-2: Self-Precedence is a Consequence, Not a Rule

There is NO special semantic rule for `self` as a receiver. `self` is treated exactly like any other parameter in provenance candidate analysis.

The common pattern where method returns tie to `self` is a **natural consequence** of LLE-1:

```rust
fn get(self: &Self) -> &T
// legal provenance candidates = [self]  (only one)
// → life_from(self)    ← consequence of LLE-1, not a special self-rule
```

When multiple receivers or parameters create ambiguity, self receives no priority:

```rust
fn compare_and_get(self: &Self, other: &Self) -> &T
// legal provenance candidates = [self, other]
// → ambiguity → explicit life_from(...) required
```

The compiler does NOT contain:
```rust
if is_method && receiver_is_ref { ... }  // FORBIDDEN
```

This aligns with the Ownership Abstraction Guard and Luna's design principle: `self` is not hardcoded as a distinguished lifetime class.

---

### LLE-3: Outlives Inference — Conservative Only

LLE-v1 infers `where outlives(a, b)` constraints **only when the relationship is directly and unambiguously entailed by the function signature**.

> [!WARNING]
> LLE-v1 does **NOT** infer outlives from nested/aggregate type structure. If a relationship requires semantic entailment reasoning beyond direct signature analysis, the programmer must write `where outlives(...)` explicitly.

**Example — no inference needed (trivially satisfied):**
```rust
fn foo(a: &A) -> &A
// single provenance, no outlives relationship needed
```

**Example — must be explicit:**
```rust
fn merge(a: &A, b: &A) -> &A
    life_from(a)
    where outlives(a, b)
// programmer explicitly states a outlives b
```

---

### LLE-4: Zero Legal Provenance Candidates → Reject

If a function's return type contains a reference but zero legal provenance candidates exist among the parameters, LLE rejects the signature:

```
error: returned reference has no legal provenance source
  --> fn bad() -> &i32
     No parameter provides a valid provenance origin for the returned reference.
     Add an explicit life_from(...) or redesign the API.
```

**Exception**: An explicit non-parameter provenance rule (if such a mechanism is ever introduced) would suppress this rejection. For LLE-v1, no such mechanism exists.

---

### LLE-5: Explicit Contract Always Overrides Inference

When the programmer writes an explicit `life_from(...)` or `where outlives(...)`, the explicit declaration is the final semantic contract. LLE does not second-guess, weaken, or strengthen an explicit annotation.

```rust
fn foo(a: &A) -> &A life_from(a)
// explicit → LLE produces life_from(a), identical to what programmer wrote
```

---

### LLE-6: Per-Dimension Suppression

> [!IMPORTANT]
> This is the key correction from the original proposal. Explicit annotations suppress inference **per dimension**, NOT globally.

LLE operates on two independent inference dimensions:
1. **Provenance inference** — deducing `life_from(...)`
2. **Outlives inference** — deducing `where outlives(...)`

Each dimension is independently suppressible:

| Explicit declaration | Inference suppressed | Inference active |
| :--- | :--- | :--- |
| `life_from(...)` | return provenance | outlives constraints |
| `where outlives(...)` | outlives constraints | return provenance |
| both | both | neither |
| neither | — | infer both (if unambiguous) |

**Example — explicit outlives does NOT suppress provenance inference:**
```rust
fn foo(a: &A, b: &B) -> &A
    where outlives(a, b)
// explicit: outlives(a, b) → suppresses outlives inference
// but: provenance candidates for return &A = [a] (single) → LLE infers life_from(a)
// result: life_from(a), where outlives(a, b)
```

**Example — explicit provenance does NOT suppress outlives inference:**
```rust
fn foo(a: &A, b: &B) -> &A
    life_from(a)
// explicit: life_from(a) → suppresses provenance inference
// outlives: if compiler can independently derive outlives constraints, it may
// result: life_from(a), plus any inferrable outlives
```

**Example — both explicit, no inference:**
```rust
fn merge(a: &A, b: &A) -> &A
    life_from(a)
    where outlives(a, b)
// both dimensions explicit → LLE produces exactly what programmer wrote
```

This design ensures LLE is an **elaboration per semantic dimension**, rather than an all-or-nothing "see explicit annotation → skip everything" heuristic.

---

### LLE-7: Elaboration Phase Contract

LLE is a distinct compiler elaboration phase positioned between parsing and semantic analysis:

```
Surface AST
    ↓
collect_provenance_candidates(...)
    ↓
infer_return_provenance(...)
    ↓
infer_outlives_constraints(...)       ← independent from provenance
    ↓
validate_explicit_contract(...)
    ↓
canonicalize_lifetime_contract(...)
    ↓
CanonicalLifetimeContract
    ↓
Borrowck / MVIR / .llib
```

`CanonicalLifetimeContract` is the **result** of elaboration, not the place where semantic analysis is performed. Downstream phases (Borrowck, MVIR, .llib serialization) consume only `CanonicalLifetimeContract` and are unaware of whether the contract was inferred or explicit.

---

## Stdlib API Classification

After LLE-v1 implementation, the stdlib can be audited into three categories:

| Category | Description | Example |
| :--- | :--- | :--- |
| **A. No borrowed return** | Function returns owned value; no lifetime annotation needed | `fn pop(&rw Self) -> Option<T>` |
| **B. Single provenance** | Exactly one legal provenance candidate; LLE elides annotation | `fn get(&Self, u64) -> Option<&T>` |
| **C. Multi-provenance / complex** | Ambiguous or multi-source; explicit annotation required | `fn splice(&Vec<T>, &Vec<T>) -> Slice<&T>` |

Current Luna stdlib is predominantly categories A and B, which means LLE-v1 will cover the vast majority of cases without requiring explicit annotations.

---

## Compatibility & Non-Breaking Guarantee

- Programs with explicit `life_from(...)` and `where outlives(...)` continue to work identically.
- Programs omitting annotations that were previously rejected may now compile (if LLE can infer).
- No existing valid program changes behavior.
- `.llib` serialization is unaffected — it always stores `CanonicalLifetimeContract`.

---

## Invariants

| ID | Invariant |
| :--- | :--- |
| **LLE-INV-1** | LLE does not create a second semantic model. `CanonicalLifetimeContract` is identical regardless of inference path. |
| **LLE-INV-2** | Explicit annotations are never weakened or overridden by inference. |
| **LLE-INV-3** | `self` receives no special treatment in the inference algorithm. |
| **LLE-INV-4** | Nested/aggregate outlives is NOT inferred in v1 without semantic entailment rules. |
| **LLE-INV-5** | Per-dimension suppression: `life_from` suppresses provenance inference only; `outlives` suppresses outlives inference only. |
| **LLE-INV-6** | Zero-candidate return references are rejected, not silently left unbounded. |

---

## Verification Plan

### Positive Tests (LLE infers correctly)
- Single-param method: `fn get(&Self) -> &T` → inferred `life_from(self)`
- Single-candidate with non-reference params: `fn get(&Self, u64) -> &T` → inferred
- Explicit + inference in different dimensions

### Negative Tests (LLE rejects or requires explicit)
- Multi-candidate ambiguity: `fn pick(&A, &A) -> &A` → error without explicit
- Zero-candidate: `fn bad() -> &i32` → error
- Complex nested: no silent outlives from aggregate structure

### Parity Tests
- Explicit `life_from(self)` produces identical `CanonicalLifetimeContract` as omitted+inferred
- `.llib` round-trip preserves contract regardless of inference path
