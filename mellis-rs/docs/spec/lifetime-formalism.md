# Luna Lifetime Relation Formal Specification (REGION-SPEC-01)

This document represents the normative mathematical and logical specification of Luna's Lifetime Relation System. While `docs/design/lifetime-model.md` defines the philosophical and architectural foundation (the "why" and the "what"), this document defines the formal constraints and solving rules (the "how"). It serves as the blueprint for the `REGION-01` compiler engine.

## 1. Region Algebra

A **Region** ($R$) is a formal validity boundary. It is categorized algebraically:

```text
Region ::= 
    // Region Constants / Bounds
    Program
    Lexical(scope_id)
    Iteration(loop_id)
    External(boundary_id)

    // Region Variables
    Contract
    Inference(var_id)
```

- **Program:** Canonical root lifetime (top element, maximum validity domain).
- **Lexical:** Concrete function body validity domain (blocks, local scopes).
- **Iteration:** Dynamic iteration family boundary (instantiated per dynamic traversal).
- **External:** Foreign / cross-boundary validity domain (FFI boundary).
- **Contract:** Abstract caller-facing API validity variable (parameters, return, self, struct fields in contracts).
- **Inference:** Callee-local inferred validity variable (temporary reference lifetimes for non-lexical lifetimes / NLL).



> **Invariant: Static Place ≠ Dynamic Lifetime Instance**  
> `Iteration(loop_id)` represents a static iteration family boundary: each dynamic traversal of the loop instantiates a distinct lifetime instance. Static compiler IR never enumerates concrete runtime iteration indices.

**Relation: Outlives ($\succeq$)**
The relation $\succeq : Region \times Region$ denotes that one region provides validity for all usages requiring the other.
- $R_a \succeq R_b$ implies $R_a$ outlives or is equivalent to $R_b$.

## 2. Constraint System

The Lifetime Relation Graph is constructed exclusively through constraint edges.

> **Normalization Principle:**  
> While the formal specification acknowledges surface relations like Binding, Projection, and Equality, these are normalized during constraint generation into canonical `Outlives` ($\succeq$) constraints before insertion into the canonical solver graph.

### 2.1 Outlives Constraint
The fundamental directed edge in the graph.

**Rule:** `OUTLIVES`
**Formal:** $R_a \succeq R_b$
**Meaning:** $R_a$ outlives $R_b$. Any reference bound to $R_a$ is valid where $R_b$ is expected.

### 2.2 Equality Constraint
Equality is normalized into bidirectional outlives constraints:

**Rule:** `EQUALITY`
**Formal:** $R_a = R_b \iff R_a \succeq R_b \land R_b \succeq R_a$
**Meaning:** $R_a$ and $R_b$ represent the exact same validity bounds.

### 2.3 Binding Constraint
When a reference is stored within a composite type, it is normalized to an equality / outlives relation between the field region and the source region.

**Rule:** `BINDING`
**Formal:** $region(Holder.value) = R_x \implies region(Holder.value) \succeq R_x \land R_x \succeq region(Holder.value)$
**Meaning:** The lifetime of the internal value is bound to Region $R_x$.

### 2.4 Projection Constraint
When extracting a lifetime from a field or variable for another use, it is normalized during constraint generation:

**Rule:** `PROJECTION`
**Formal:** $R_{return} \preceq R_{holder.value}$ (via `life_from(holder.value)`)
**Meaning:** The return region is constrained to not outlive the region of the projected field.


## 3. Constraint Generation Rules

The compiler translates AST constructs and user contracts into the formal constraint system.

A Reference value in Luna is formally defined as a 3-tuple:
```text
Reference = (Provenance, Region Constraint, Capability)
```
where:
- **Provenance**: The originating Place identity and associated loan.
- **Region Constraint**: The validity domain bound ($R_{ref} \preceq R_{source}$).
- **Capability**: The access permission ($\&$ shared/read-only or $\&rw$ exclusive/read-write).

### 3.1 Reference Creation
When a reference is created to a Place.

**Rule:** `REF-CREATE`
**Formal:** 
```text
dec r = &x
```
**Generate:**
- $loan(x) = L_x$
- $provenance(r) = x$
- $R_r \preceq R_x$
- $capability(r) = Shared$

**Meaning:** Creating a reference establishes a loan, sets provenance, assigns shared capability, and constrains the reference's validity to not exceed the region of the borrowed Place.

### 3.2 Mutable Reference
When a rewritable reference is created.

**Rule:** `MUT-REF-CREATE`
**Formal:**
```text
dec r = &rw x
```
**Generate:**
- $loan(x, Mutable) = L_x^{rw}$
- $provenance(r) = x$
- $R_r \preceq R_x$
- $capability(r) = Exclusive$
- Constraint: $\forall p \in R_r, \nexists \text{ conflicting loan on } x$

**Meaning:** While the region $R_r$ is active, no conflicting loans may exist for $x$.

### 3.3 Provenance Contract (`life_from`)
Mapping explicit boundary contracts to constraints.

**Rule:** `CONTRACT-LIFE-FROM`
**Formal:**
```text
fn f(x: &T) -> &T
    life_from(x)
```
**Generate:**
- $R_{return} \preceq R_x$

**Meaning:** The return value acquires its provenance bounds from the source $x$.

### 3.4 Outlives Contract (`requires`)
Mapping explicit relation contracts.

**Rule:** `CONTRACT-REQUIRES`
**Formal:**
```text
requires life(a) >= life(b)
```
**Generate:**
- $R_a \succeq R_b$

**Meaning:** The region of $a$ is explicitly constrained to outlive the region of $b$.

## 4. Solver Rules (Propagation)

The graph engine resolves the system of constraints to determine validity.

### 4.1 Transitivity
**Rule:** `OUTLIVES-TRANSITIVE`
**Formal:**
$$ \frac{\Gamma \vdash R_a \succeq R_b \quad \Gamma \vdash R_b \succeq R_c}{\Gamma \vdash R_a \succeq R_c} $$
**Meaning:** If region A outlives B, and B outlives C, then A outlives C.

## 5. Solver Failure Model

A cycle in the constraint graph ($R_a \succeq R_b \land R_b \succeq R_a$) is **not** an error by itself, as it simply denotes equality ($R_a = R_b$). 

### 5.0 Decoupling Invariant: Region Validity vs. Capability Validity
$$\text{ValidUse} = \text{RegionSatisfied} \land \text{CapabilityValid}$$

- **`RegionUnsatisfied` (Symbolic Region Engine domain):** The constraint graph combined with known static region bounds and usage requirements yields a logical contradiction ($\bot$).
- **`CapabilityViolation` (Borrowck / Loan Engine domain):** Shared vs. exclusive aliasing or simultaneous conflicting loans occur on the same Place.

The Region Solver strictly verifies **`RegionSatisfied`** and does **not** evaluate capability exclusivity or loan conflicts.

**Definition:** 
$$InvalidGraph = \text{ConstraintGraph} \cup \text{RegionBounds} \cup \text{UsageRequirements} \implies \bot$$


### 5.1 Bound Violation Example (Invalid Promotion)
**Rule:** `INVALID-PROMOTION`
**Formal:** $R_{local} \succeq R_{program}$
**Meaning:** Attempting to force a local scope to outlive the entire program. This contradicts the fundamental bounds of $R_{program}$.
**Diagnostic:** `E30xx: local region cannot satisfy program lifetime bounds`

### 5.2 Escaping Closure Error (E3005)
When a closure captures a local variable but the closure escapes the local scope.

**Rule:** `CLOSURE-ESCAPE`
**Code:** 
```rust
fn create() -> fn() {
    dec x = 10;
    return || x;
}
```
**Constraint Generated:** $R_{captured(x)} \succeq R_{closure}$
**Bounds Analysis:**
- Invariant: Captured provenance must outlive the closure that accesses it ($R_{captured} \succeq R_{closure}$).
- Here, $R_{captured(x)} = R_{local(create)}$, which terminates upon return from `create()`.
- The returned closure escapes into the caller's scope, requiring $R_{closure} \succeq R_{caller}$.
- Therefore, forcing $R_{local(create)} \succeq R_{closure}$ is mathematically unsatisfiable (a destroyed local scope cannot outlive an escaping closure).

**Diagnostic:** `E3005: closure lifetime exceeds captured provenance`


---

## Appendix A: Region Ordering ($\succeq$ as Validity Containment)

A critical conceptual foundation of Luna is that region ordering does **not** represent physical chronological time or storage lifespan:

$$R_a \succeq R_b \iff \text{ValidUses}(R_b) \subseteq \text{ValidUses}(R_a)$$

Equivalently:
> **"$R_b$ requires at most what $R_a$ provides."**

- **Validity Domain:** A region is an abstract set of control-flow points where a reference may be validly dereferenced.
- **Subtyping / Capability Provision:** If $R_a \succeq R_b$, any obligation satisfied by $R_a$ also satisfies $R_b$. A reference valid across $R_a$ can safely be used in any context requiring $R_b$.
- **Storage Disjointness:** Storage denotes the physical lifetime of memory allocation on stack or heap. Region denotes the logical validity domain of a reference. Two variables may share a static `Place` across loop iterations, but each iteration establishes a distinct dynamic Region instance.

---

## Appendix B: Reference Validity Judgment

A reference is valid in typing context $\Gamma$ if and only if all three components of its tuple are independently verified:

$$\frac{\Gamma \vdash \text{Provenance}(ref) \quad \Gamma \vdash \text{Region}(ref) \quad \Gamma \vdash \text{Capability}(ref)}{\Gamma \vdash ref : T \text{ valid}}$$

Each premise directly corresponds to compiler diagnostic categories:

| Premise | Validation Rule | Diagnostic Category on Failure |
| :--- | :--- | :--- |
| $\Gamma \vdash \text{Provenance}(ref)$ | The reference originates from a known, live `Place` whose storage has not been deallocated or invalidated. | Dangling reference / use-after-free (e.g., `E3001: reference to dropped storage`). |
| $\Gamma \vdash \text{Region}(ref)$ | The reference satisfies all relation constraints ($R_{ref} \preceq R_{source}$) across its active CFG span without bound contradiction. | Lifetime outlives violation / escape error (e.g., `E3005: closure lifetime exceeds captured provenance`). |
| $\Gamma \vdash \text{Capability}(ref)$ | Access permissions ($\&$ vs $\&rw$) adhere to exclusivity and non-aliasing invariants across the reference's region. | Mutability / borrow conflict (e.g., `E3002: cannot borrow place as rw while already borrowed`). |

---

## Appendix C: Solver Guarantees

The Luna Lifetime Relation Engine commits to the following formal properties:

### C.1 Soundness
If the constraint solver accepts a program under context $\Gamma$, then no invalid reference usage, dangling pointer dereference, or mutable aliasing can occur during well-typed execution:

$$\text{SolverAccept}(\mathcal{G}) \implies \forall r \in \text{References}, \quad \text{ActiveUses}(r) \subseteq \text{ValidityDomain}(\text{Provenance}(r))$$

The symbolic region solver verifies that no reference outlives its allowed validity domain; capability exclusivity is verified independently by the borrow checker.

### C.2 Completeness (Conservative Approximation)
The solver is **sound but deliberately conservative**:

$$\text{SafePrograms} \supset \text{AcceptedPrograms}$$

Because static control-flow analysis and path-sensitivity are subject to undecidability (the halting problem), the engine does not guarantee accepting every theoretically safe program. If an edge cannot be proved sound from available constraints and static bounds, the solver rejects the construct. Luna explicitly prefers conservative rejection over unsound acceptance.

