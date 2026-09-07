# Mellis Normative Language Rules: Foundations (P0 & P1)

## Status: Frozen Draft  
**Context**: Refined following *Mellis Language Semantic Audit (Specification-Level)*.  
**Purpose**: Establish four foundational, mathematically sound normative rules to serve as the unshakeable source of truth for the Mellis type system, borrow checker, and compiler architecture.

---

## Rule A: Mutual Exclusivity of `Copy` and `Drop` (P0)

### A.1 Core Invariant
$$\forall T \in \text{ConcreteTypes}, \quad \neg (\text{satisfies}(T, \text{Copy}) \land \text{satisfies}(T, \text{Drop}))$$

A concrete type cannot satisfy both `Copy` and `Drop`.

### A.2 Rationale
`Copy` guarantees that bitwise duplication of a value's memory footprint produces a distinct, fully independent instance with zero unique resource ownership obligations. `Drop` guarantees that a value owns unique external resources requiring custom finalization logic executed upon scope exit. Permitting a concrete type to satisfy both creates an irreconcilable soundness paradox: bitwise copying duplicates the instance without invoking lifecycle hooks, leading to guaranteed double-free or resource corruption when the destructor executes on multiple bitwise copies.

### A.3 Enforcement Paths
1. **Direct Implementation**:
   - Given `impl Drop for T`, any declaration `impl Copy for T` (or vice-versa) is a static compile-time error (`E_COPY_DROP_CONFLICT`).
2. **Derived Implementation**:
   - `#[derive(Copy)]` on a struct or enum is rejected if the type itself implements `Drop`.
3. **Transitive Field Constraints**:
   - A composite type (struct, enum, tuple) cannot satisfy or derive `Copy` if any transitive field $f_i$ satisfies `Drop`.
4. **Separation from Generic Bounds**:
   - The invariant applies strictly to **concrete types**.
   - A generic bound such as `fn foo<T: Copy + Drop>(x: T)` specifies an impossible constraint for any valid Mellis type. Whether the compiler rejects this bound at definition-time (via an optional bound-consistency checker) or at instantiation-time (when no concrete type can satisfy both) is a diagnostic design choice, and does not alter the core invariant.

---

## Rule B: Prohibition of Subplace Moves under `Drop` (P0)

### B.1 Core Invariant
$$\forall \text{Place } p, \forall \text{Place } a \text{ such that } a \sqsubset p, \quad \text{satisfies}(\text{typeof}(a), \text{Drop}) \implies \neg\text{can\_move}(p)$$

A proper subplace of a `Drop` place cannot be moved out. Whole-place move remains legal.

### B.2 Rationale
When a place $a$ of a type implementing `Drop` goes out of scope, the destructor hook `Drop::drop(&rw a)` must execute on a fully valid, fully initialized memory place. Moving a proper subplace $p = a.\text{field}$ leaves $a$ in a partially deinitialized state. Invoking `Drop::drop` on a partially deinitialized struct causes undefined behavior.

### B.3 Place Semantics
1. **Proper Subplace Move**:
   - Given a place $c$ whose type satisfies `Drop`: evaluating `move c.field` (by variable binding, function argument, or pattern match) is **strictly forbidden** (`E_PARTIAL_MOVE_UNDER_DROP`).
2. **Hierarchical Propagation**:
   - If $a \sqsubset p$ (i.e., $a$ is any strict prefix or ancestor place of $p$) and $\text{typeof}(a)$ satisfies `Drop`, moving $p$ is forbidden.
3. **Whole-Place Move**:
   - Moving the entire container $c$ (`dec a = c;`) transfers full ownership of all fields and shifts the destructor obligation to the new binding. **Whole-place move is fully permitted.**
4. **Field Borrowing**:
   - Borrowing a subplace (`&c.field` or `&rw c.field`) does not deinitialize the container. It is **fully permitted** under normal non-lexical borrow check rules.
5. **Field Reassignment / Overwrite**:
   - Evaluating `c.field = new_value;` on an already-initialized container $c$ does not produce a partially moved state. The previous field value is dropped in-place and replaced with `new_value`. The container $c$ remains fully initialized. **Field assignment is fully permitted.**

---

## Rule C: Associated Type Projections & The Monomorphization Barrier (P0)

### C.1 Core Invariant
Associated type projections may remain symbolic during generic semantic analysis, but no unresolved projection may cross the monomorphization boundary into MVIR and backend code generation.

### C.2 Rationale
During generic typechecking and constraint solving, associated types (such as `<T as Try>::Output` or `<T as Try>::Residual`) cannot always be normalized immediately because the concrete type argument for `T` is unknown. Demanding premature normalization before borrow checking would cripple generic analysis. However, downstream phases (monomorphic MVIR lowering, memory layout calculation, LLVM code generation) require completely concrete types.

### C.3 Phase-by-Phase Lifecycle
```
Source Generic AST
       │
       ▼
Generic Semantic Analysis ─────► Projections (<T as Trait>::Assoc) may remain symbolic
       │                         Obligations & equality constraints (T::Assoc == U) recorded
       ▼
Trait Solving & Normalization ──► Normalized eagerly where concrete impls are known
       │
       ▼
=== MONOMORPHIZATION BARRIER ===
       │
       ▼
Monomorphic MVIR & Borrowck ───► STRICT INVARIANT: Zero unresolved projections,
                                 zero GenericParam, and zero InferenceVar allowed.
```

### C.4 Soundness of the `Try` Protocol
For the desugaring of `val?`:
1. In a generic function with `T: Try`, the branch return type is symbolically `<T as Try>::Residual` and `<T as Try>::Output`.
2. The caller return constraint is unified: $\text{ReturnType} : \text{FromResidual}<<T \text{ as Try}>::\text{Residual}>$.
3. Upon monomorphization with concrete type $T = \text{Result}<i32, \text{Error}>$, the projections normalize to:
   - `<T as Try>::Output` $\longrightarrow i32$
   - `<T as Try>::Residual` $\longrightarrow \text{Result}<\text{Infallible}, \text{Error}>$
4. Full type soundness is verified prior to MVIR emission.

---

## Rule D: Trait Coherence & The Nominal Head Orphan Rule (P1)

### D.1 Core Invariant
$$\forall (\text{Trait}, \text{Type}) \text{ in program}, \quad \text{applicable\_impls}(\text{Trait}, \text{Type}) \le 1$$

For every canonical $(\text{Trait}, \text{Type})$ pair in a program, there is at most one applicable implementation.

### D.2 The Nominal Head Orphan Rule
An implementation `impl<...> Trait for TargetType` is legal within provider $P$ if and only if at least one of the following conditions is satisfied:

1. **Local Trait**: The `Trait` is declared within provider $P$.
2. **Local Nominal Head Type**: The root constructor (nominal head) of `TargetType` is declared within provider $P$.

### D.3 Precise Rules for Generic Wrappers
Local ownership of an implementation is governed exclusively by the **nominal head** of the self type, never by its generic type parameters:

| Implementation Declaration | Provider of `Wrapper` | Provider of `T` | Legal in Provider $P$? | Reason |
| :--- | :--- | :--- | :--- | :--- |
| `impl<T> ExternalTrait for LocalWrapper<T>` | Local ($P$) | External | **YES** | `LocalWrapper` is the local nominal head. |
| `impl<T> ExternalTrait for ExternalWrapper<LocalType>` | External | Local ($P$) | **YES** | Covered if target type contains a local nominal type within the first non-fundamental level. |
| `impl<T> ExternalTrait for ExternalWrapper<T>` | External | Generic Param | **NO (REJECT)** | No local nominal identity. Adding this would create potential collisions. |
| `impl ExternalTrait for ExternalType` | External | External | **NO (REJECT)** | Pure orphan implementation. Strictly forbidden. |

### D.4 Consequence for Language Extensibility
* A user cannot write `impl Try for i32` or `impl Drop for String` because neither the trait nor the type head is local.
* A user can write `impl Try for MyResult<T, E>` because `MyResult` is a local nominal type.
* This guarantees that independent packages can be compiled and linked together without silent implementation collisions or coherence breakdowns.
