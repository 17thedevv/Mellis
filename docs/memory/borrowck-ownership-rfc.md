# Mellis Compiler RFC: Borrowck Ownership, Move & Drop Rules (P6 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P6 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P6)
- **Scope**: Ownership Semantics, Place Tree State, Move Analysis, Borrow Rules, `has_user_drop` vs `needs_drop`, Reinitialization via Initialize vs Write, `overlaps` Relation, Drop Elaboration, Subobject Drop Flags, Realloc Invalidation, Diagnostic Codes
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Borrowck & Ownership Verification ("When are memory operations legal in safe code?")

---

## 1. Architectural Role & Boundaries

Following the freeze of **P0 (Memory Model)**, **P1 (Layout Model)**, **P2 (Pointer & Provenance)**, **P3 (Allocator Contract)**, **P4 (Runtime ABI)**, and **P5 (MVIR Operations)**:

> **P6 Core Mandate**:
> P6 does **NOT** redefine memory, storage, or runtime operations.
> P6 proves at compile-time that every memory operation emitted in MVIR adheres to the **laws of ownership, aliasing, and destruction**:
>
> 1. Every owned move-only resource has **at most one** owning place at any program point (no double-free).
> 2. Values are never accessed after being moved out (no use-after-move).
> 3. Aliasing XOR Mutability is strictly maintained via exclusive access rules (no data races).
> 4. Initialized objects are dropped exactly once before storage is deallocated (no leak, no double-drop).
> 5. Dropping is statically or dynamically scheduled without manual programmer intervention.

```text
                Ownership Facts
                      │
          ┌───────────┴───────────┐
          │                       │
     Place Tree              Loan Set
          │                       │
          ▼                       ▼
    field/init state         active borrows
          │                       │
          └───────────┬───────────┘
                      ▼
            Dataflow MemoryState
                      │
                      ▼
               Drop Elaboration
                      │
                      ▼
               Safe Lowered MVIR
```

---

## 2. Ownership & Value Categories

Mellis classifies types into two fundamental ownership models:

### 2.1 Copy Types
- **Characteristics**: Bitwise duplication. Value cloning leaves the source place active and unchanged.
- **Types**: Primitive scalars (`i8`..`i128`, `u8`..`u128`, `f32`, `f64`, `bool`, `char`), raw pointers (`*T`, `*rw T`), shared references (`&T`), and aggregate types composed exclusively of `Copy` types that do not define custom drop glue.
- **Invariants**: `needs_drop(T) == false`. `DropInPlace` is never scheduled. Reads use `Instruction::Load` (or `RawLoad`).

### 2.2 Move-Only Types
- **Characteristics**: Affine / Linear ownership. Moving transfers exclusive resource ownership to the destination, consuming the source place.
- **Types**: Heap containers (`Box<T>`, `RawVec<T>`, `Vec<T>`, `String`), types with custom destructors (`impl Drop`), and aggregate types containing at least one move-only field.
- **Invariants**: Moving a value uses `Instruction::MoveOut`, transitioning the source place to `Uninitialized` / `Moved`.

---

## 3. Place Tree Model & Projections

Memory places are modeled hierarchically as a **Place Tree**:

```text
┌────────────────────────────────────────────────────────┐
│                        Place                           │
├───────────────────────────┬────────────────────────────┤
│ local: ValueId            │ projections: Vec<Proj>     │
└───────────────────────────┴────────────────────────────┘
```

### 3.1 Projections
```rust
pub enum Projection {
    /// Field indexing into struct, tuple, or variant: `x.f`
    Field(usize),
    /// Element indexing into array or slice: `x[i]`
    Index(Operand),
    /// Pointer or reference dereference: `*x` or `*rw x`
    Deref,
}
```

### 3.2 Place State Tree (Derived Summary vs Leaf Facts)
Place state is tracked recursively through the projection tree:
- **Leaf Places** (scalars, primitives, terminal fields) hold discrete dataflow states:
  $$\text{LeafState} \in \{ \text{Uninitialized}, \text{Initialized}, \text{Moved} \}$$
- **Aggregate Places** (structs, tuples) have their status **derived** from their descendants:
  $$\text{state}(P) = \begin{cases}
  \text{Initialized} & \text{if } \forall C \in \text{children}(P): \text{state}(C) == \text{Initialized} \\
  \text{Uninitialized} & \text{if } \forall C \in \text{children}(P): \text{state}(C) == \text{Uninitialized} \\
  \text{Moved} & \text{if } \forall C \in \text{children}(P): \text{state}(C) == \text{Moved} \\
  \text{PartiallyMoved} & \text{otherwise (some children Initialized, some Moved)}
  \end{cases}$$

`PartiallyMoved` is a **derived summary status**, not a raw leaf state. This enables field-sensitive drop elaboration: when dropping an aggregate place, the compiler drops only those individual subfields where $\text{state}(C) == \text{Initialized}$.

---

## 4. Assignment Semantics: Initialize vs. Write

Borrowck resolves the semantic distinction between assignment to uninitialized storage and assignment to an active object:

### 4.1 Assignment Resolution Rule
When an assignment `P = val` occurs in source code:

$$\text{Assignment}(P, val) \implies \begin{cases}
\text{Instruction::Initialize}(P, val) & \text{if } \text{state}(P) \in \{ \text{Uninitialized}, \text{Moved} \} \\
\text{Instruction::Write}(P, val) & \text{if } \text{state}(P) == \text{Initialized}
\end{cases}$$

1. **`Initialize` (Reinitialization)**:
   - Invoked when $P$ has no active instance.
   - Transitions $P$ from `Uninitialized` / `Moved` to `Initialized`.
   - Generates zero drop calls (there is no previous value to destroy).
2. **`Write` (Overwriting Active Place)**:
   - Invoked when $P$ is currently `Initialized`.
   - Lowers to: `DropInPlace(P, T)` (if `needs_drop(T)`), followed by `Initialize(P, val)`.
   - Place remains `Initialized`.

---

## 5. Partial Moves: `has_user_drop` vs. `needs_drop`

Mellis cleanly distinguishes types with explicit custom destructors from types that merely contain drop-requiring fields.

### 5.1 The Two Drop Predicates
1. **`has_user_drop(T) -> bool`**:
   `true` if and only if type $T$ has an explicit implementation of trait `Drop` (`impl Drop for T`).
2. **`needs_drop(T) -> bool`**:
   `true` if `has_user_drop(T)` is true OR if any recursive subfield requires destruction.

### 5.2 The Partial Move Invariants (MOVE-PARTIAL-1 & 2)

- **MOVE-PARTIAL-1 (Prohibition Under User Drop - `E3002`)**:
  If $\text{has\_user\_drop}(T) == \text{true}$, moving a field out of $T$ (`let x = container.field;`) is **statically forbidden**:
  $$\text{has\_user\_drop}(T) \implies \text{Partial moves out of } T \text{ are REJECTED} \ (\text{DiagnosticCode::PartialMoveUnderDrop}, \text{E3002})$$
  *Rationale*: A user-defined destructor `Drop::drop(&rw self)` expects a complete, structurally intact instance of $T$. Partial moves would leave `self` partially empty during destruction.
- **MOVE-PARTIAL-2 (Permitted for Aggregate Types without User Drop)**:
  If $\text{needs\_drop}(T) == \text{true} \land \text{has\_user\_drop}(T) == \text{false}$ (e.g. `struct Pair { a: String, b: String }`):
  **Partial moves ARE PERMITTED**:
  ```rust
  dec p = Pair { a: ..., b: ... };
  dec first = p.a; // Legal! p.a is Moved, p.b is Initialized
  ```
  During drop elaboration, the compiler tracks subfield states: it drops `p.b` and elides destruction of `p.a`.

---

## 6. The `overlaps` Relation & Borrow Rules

### 6.1 Formal Overlap Relation: `overlaps(P1, P2) -> bool`

Two places $P_1$ and $P_2$ overlap if an access to one can affect or alias the other:

$$\text{overlaps}(P_1, P_2) \iff \begin{cases}
\text{true} & \text{if } P_1 == P_2 \\
\text{true} & \text{if } P_1 \text{ is an ancestor of } P_2 \lor P_2 \text{ is an ancestor of } P_1 \\
\text{false} & \text{if } P_1 = x.f \land P_2 = x.g \text{ where } f \ne g \text{ (disjoint sibling fields)} \\
\text{true} & \text{if both dereference pointers that may alias}
\end{cases}$$

### 6.2 Borrowing Rules (Aliasing XOR Mutability)

A loan $L$ on place $P_{loan}$ grants an access capability.

- **Shared Loan (`&T`)**:
  - Grants concurrent read access.
  - While active, any requested access $P_{req}$ that satisfies $\text{overlaps}(P_{req}, P_{loan})$:
    - Shared reads through $P_{loan}$ are **ALLOWED**.
    - Mutations, writes, mutable borrows, and moves on overlapping places are **REJECTED** (`E3003 BorrowConflict`).
- **Mutable Loan (`&rw T`)**:
  - Grants **exclusive access** to the referent.
  - The holder of the mutable reference may **freely read and write through the reference itself** (`*r`, `*rw r`).
  - While active, all *other* accesses, reborrows, reads, writes, or moves through any place overlapping $P_{loan}$ are **REJECTED** (`E3003 BorrowConflict`).

---

## 7. Drop Elaboration & Subobject Drop Flags

Drop elaboration converts variable lifetimes into exact, minimal destruction calls.

### 7.1 Static Drop Elision
At scope exit / function return:
- For every place $P$: if dataflow analysis statically proves $\text{state}(P) == \text{Initialized}$, emits `DropInPlace(P, T)`.
- If $\text{state}(P) \in \{ \text{Uninitialized}, \text{Moved} \}$, emits zero instructions.
- If $\text{needs\_drop}(T) == \text{false}$, emits zero instructions.

### 7.2 Subobject Dynamic Drop Flags
When a place or subfield is conditionally moved across CFG branches:

$$\text{Path 1}: \text{consume}(p.a) \quad \big| \quad \text{Path 2}: \text{(no move)}$$

At the join point, $p.a$ is in state `MaybeInitialized`.
1. The compiler synthesizes a boolean drop flag for that subobject: `drop_flag_p_a: bool`.
2. Initialized: `drop_flag_p_a = true`.
3. Moved out: `drop_flag_p_a = false`.
4. At destruction point:
   ```text
   BranchIf drop_flag_p_a, bb_drop_a, bb_skip_a
   bb_drop_a:
       DropInPlace { ptr: &p.a, ty: TypeOf(p.a) }
       Branch bb_skip_a
   ```

---

## 8. CFG Dataflow Lattice & Loop Invariants

### 8.1 Per-Place Lattice Merge Rules
At CFG join points, dataflow states merge per leaf place:

| State on Path 1 | State on Path 2 | Merged State at Join | Action Required |
|:---|:---|:---|:---|
| `Initialized` | `Initialized` | `Initialized` | Static drop |
| `Moved` | `Moved` | `Moved` | Static elision |
| `Uninitialized` | `Uninitialized` | `Uninitialized` | Static elision |
| `Initialized` | `Moved` | `MaybeInitialized` | Requires dynamic drop flag |
| `Initialized` | `Uninitialized` | `MaybeInitialized` | Requires dynamic drop flag |

### 8.2 Loop Invariants
A variable cannot be moved inside a loop body if it is read on a subsequent iteration without an intervening reinitialization. Attempting to consume an un-reinitialized variable across loop back-edges emits `DiagnosticCode::UseAfterMove` (`E3001`).

---

## 9. Reallocation Invalidation Interaction

When `Instruction::Realloc { ptr, ... }` is executed:

> **Realloc Borrow Invalidation Invariant**:
> Reallocating an allocation $A$ invalidates all loans whose referents overlap storage in $A$.
>
> If Borrowck detects any active borrow whose lifetime extends across a `Realloc` instruction on the containing allocation, the program is **statically rejected** with `DiagnosticCode::BorrowConflict` (`E3003`):

```text
error[E3003]: cannot reallocate buffer while actively borrowed
  --> src/vec.ms:45:9
   |
42 |     dec elem = &vec.buf[0];
   |                ----------- active borrow on buffer element begins here
45 |     vec.grow(); // executes Realloc
   |     ^^^^^^^^^^ reallocation invalidates existing buffer loans
46 |     print(*elem);
```

---

## 10. Diagnostic System v1.0 Conformance

All Borrowck violations map 1-to-1 to the frozen Diagnostic Registry:

| Code | Name | Primary Label | Related Label (Origin Tracking) |
|:---|:---|:---|:---|
| `E3001` | `UseAfterMove` | Access to moved place | Prior `MoveOut` instruction location |
| `E3002` | `PartialMoveUnderDrop` | Field move expression | Struct `impl Drop` declaration |
| `E3003` | `BorrowConflict` | Conflicting loan or access | Active existing loan definition |
| `E3004` | `MissingReturnValue` | Closing brace of non-void fn | Function return type annotation |
| `E3005` | `LocalBorrowEscape` | Escaping return expression | Local stack variable declaration |

---

## 11. Summary & Transition Criteria to P7

With this P6 Hardened Specification:
1. `has_user_drop` is decoupled from `needs_drop`, allowing valid partial moves in aggregate types.
2. Assignment to moved places is cleanly routed to `Initialize`, resolving the reinitialization contradiction.
3. The **Place State Tree** provides true field-sensitive state tracking.
4. The **`overlaps` relation** enables fine-grained disjoint field borrow checking.
5. Mutable loans explicitly allow access through the loan itself while forbidding external aliases.
6. Subobject drop flags support fine-grained conditional destruction.
7. Realloc invalidates loans overlapping the affected allocation.
8. Diagnostic codes `E3001`–`E3005` are fully synchronized.

Upon review and approval, the Tier 1 roadmap advances to **P7: End-to-End Conformance & Executable Verification**.
