# Mellis Compiler RFC: MVIR Decomposed Memory Operations (P5 Hardened v2)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P5 Hardened v2)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P5)
- **Scope**: MVIR Intermediate Representation, 7 Primitive Memory Operations, Realloc First-Class Primitive, Path-Sensitive MemoryState, AllocationSiteId vs AllocId, Effect Model, MoveOut vs Load, Verifier Invariants
- **Authority**: Canonical Single Source of Truth (SSOT) for MVIR Memory Operations ("How does the compiler represent memory lifecycles in IR?")

---

## 1. Architectural Purpose & Boundaries

Following the freeze of **P0 (Memory Model)**, **P1 (Layout Model)**, **P2 (Pointer & Provenance)**, **P3 (Allocator Contract)**, and **P4 (Runtime ABI)**, P5 formalizes the compiler's middle-end intermediate representation:

> **P5 Core Mandate**:
> MVIR explicitly represents the **discrete semantic lifecycle events** of memory storage and typed objects across control-flow graphs (CFG).
>
> Monolithic black-box instructions (`BoxNew`, `BoxFree`, untyped `HeapAlloc`) and generic FFI bypasses (e.g. treating reallocation as an opaque `CallDirect`) are permanently eliminated.
> Memory lifecycles are governed by **7 orthogonal, primitive MVIR operations**:
> $$\text{Alloc} \ \ \big| \ \ \text{Realloc} \ \ \big| \ \ \text{Dealloc} \ \ \big| \ \ \text{Initialize} \ \ \big| \ \ \text{Write} \ \ \big| \ \ \text{MoveOut} \ \ \big| \ \ \text{DropInPlace}$$

```text
                   MVIR Subsystem
                         │
           ┌─────────────┴─────────────┐
           │                           │
    AllocationFact                MemoryState
 (immutable metadata)         (path-sensitive dataflow)
           │                           │
           └─────────────┬─────────────┘
                         │
             Structured Effect System
                         │
            ┌────────────┼────────────┐
            │            │            │
         Memory     Provenance      Drop
            │            │            │
            └────────────┴────────────┘
                         │
              LLVM Backend Lowering
```

---

## 2. The 7 Primitive Memory Operations

### 2.1 MVIR Instruction Definitions

```rust
pub enum Instruction {
    // --- Primitive Storage Operations -----------------------------------------
    /// Requests raw uninitialized storage of `size` bytes aligned to `align`.
    /// Emitted at static `site: AllocationSiteId`.
    /// Produces a raw storage handle of type `*rw u8`.
    Alloc {
        site: AllocationSiteId,
        size: Operand,
        align: Operand,
    },

    /// Resizes an active allocation, replacing its allocation identity.
    /// Operates as a first-class memory primitive (NOT a generic CallDirect).
    /// On success: old AllocId -> Terminated, fresh AllocId -> Active.
    /// On failure: old AllocId remains Active.
    Realloc {
        ptr: Operand,
        old_size: Operand,
        old_align: Operand,
        new_size: Operand,
    },

    /// Releases raw storage previously allocated via `Alloc` or `Realloc`.
    /// Precondition: Place must be in `ALLOCATED / UNINITIALIZED` state.
    /// Transitions allocation dataflow state to `Terminated`.
    Dealloc {
        ptr: Operand,
        size: Operand,
        align: Operand,
    },

    // --- Typed Object Lifecycle Operations ------------------------------------
    /// Moves ownership of `value: T` into uninitialized storage at `dst`.
    /// Precondition: `dst` must be in `ALLOCATED / UNINITIALIZED` state.
    /// Transitions `dst` to `INITIALIZED`. Does NOT drop prior contents.
    Initialize {
        dst: Operand,
        value: Operand,
    },

    /// Overwrites an already initialized place `dst` with `value: T`.
    /// Precondition: `dst` must be in `INITIALIZED` state.
    /// Atomic with respect to MVIR ownership/lifecycle state (NOT a CPU atomic op).
    /// Destructs prior contents (if `needs_drop(T)`), then writes new value.
    Write {
        dst: Operand,
        value: Operand,
    },

    /// Extracts the value of type `T` out of `src`, transferring ownership to caller.
    /// Precondition: `src` must be in `INITIALIZED` state.
    /// Transitions `src` to `ALLOCATED / UNINITIALIZED`.
    /// Distinct from `Load`: `MoveOut` consumes ownership; `Load` copies.
    MoveOut {
        src: Operand,
    },

    /// Executes canonical destruction glue for type `ty` on the instance at `ptr`.
    /// Precondition: `ptr` must be in `INITIALIZED` state.
    /// Transitions `ptr` to `ALLOCATED / UNINITIALIZED`.
    /// If `needs_drop(ty) == false`, elided statically as a compile-time no-op.
    DropInPlace {
        ptr: Operand,
        ty: SemanticTypeId,
    },

    // ... (arithmetic, control flow, calls, borrows, etc.)
}
```

---

## 3. Path-Sensitive `MemoryState` vs. Immutable `AllocationFact`

In a control-flow graph with branches and loops, an allocation's state cannot be a single global mutable variable. MVIR cleanly bifurcates static identity from path-sensitive dataflow state.

### 3.1 Immutable Metadata: `AllocationFact`
Registered when an allocation site is defined:

```rust
pub struct AllocationFact {
    pub id: AllocId,
    pub site: AllocationSiteId,
    pub layout: Layout,
    pub is_zst: bool,
    pub allocator_domain: AllocatorDomain,
}
```

- **`AllocationSiteId`**: The static IR program location of the `Alloc` instruction.
- **`AllocId`**: The abstract dynamic allocation identity in the memory model. In loops or recursive calls, a single static `AllocationSiteId` generates fresh dynamic `AllocId` instances per dynamic invocation.

### 3.2 Path-Sensitive Dataflow: `MemoryState`
Tracks the active/terminated status at each program point:

```rust
pub struct MemoryState {
    pub allocations: std::collections::BTreeMap<AllocId, AllocationState>,
    pub places: std::collections::BTreeMap<PlaceId, PlaceLifecycleState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceLifecycleState {
    Uninitialized,
    Initialized,
}
```

#### CFG Branch & Join Lattice:
```text
          MemoryState [A: Active, p: Uninitialized]
                     │
                  branch
                 /      \
             Path 1    Path 2
               │          │
        Initialize(p)   (no-op)
               │          │
      [p: Initialized]  [p: Uninitialized]
                 \      /
                   join
                    │
        MemoryState [p: Conflicted / Requires DropFlag]
```
At CFG join points, states are merged via dataflow analysis. If an allocation is deallocated on one branch but not another, the verifier requires explicit drop/dealloc flags or emits a diagnostic for unbalanced deallocation.

---

## 4. MoveOut vs. Load vs. RawLoad

MVIR maintains strict distinction between high-level typed access and low-level byte reads:

| Operation | Applicability | Place State Transition | Ownership Impact |
|:---|:---|:---|:---|
| **`Instruction::MoveOut`** | Move-only / non-`Copy` types | `INITIALIZED -> UNINITIALIZED` | Moves ownership to SSA register |
| **`Instruction::Load`** (Typed) | `Copy` types only | Remains `INITIALIZED` | Clones value bits; place stays readable |
| **`Instruction::RawLoad`** | Raw pointers / untyped bytes | Unchanged (untyped) | Reads machine bits without language ownership |

---

## 5. First-Class `Realloc` Semantics & Compound Effect

`Realloc` is not an opaque function call; it is a dedicated memory primitive with compound semantic effects:

```text
Realloc { ptr, old_size, old_align, new_size }
```

### 5.1 Reallocation Invariants:
1. **Old Allocation Termination**: In dataflow state, `allocations.insert(old_id, AllocationState::Terminated)`.
2. **Fresh Allocation Creation**: Generates fresh `new_id` with `AllocationState::Active`.
3. **Old Pointer Revocation**: Any subsequent access through `ptr` fails temporal validity.
4. **Failure Atomicity**: If reallocation fails, dataflow state leaves `old_id` as `Active`.

### 5.2 Compound Memory & Provenance Effect:
```rust
pub enum MemoryEffect {
    None,
    Read,
    Write,
    Allocate,
    Deallocate,
    /// Atomic compound effect representing reallocation
    Reallocate {
        old_alloc: AllocId,
        new_alloc: AllocId,
        preserves_bytes: usize,
    },
}
```
This compound effect prevents the optimizer from decomposing `Realloc` into separate `Read + Write + Dealloc + Alloc` and reordering them across other instructions.

---

## 6. Allocation Non-Aliasing Invariant

The optimizer relies on the following formal non-aliasing rule:

> **Active Allocation Aliasing Invariant**:
> For any two distinct allocation identities $A \ne B$:
> $$\text{State}(A) == \text{Active} \ \land \ \text{State}(B) == \text{Active} \implies \text{range}(A) \cap \text{range}(B) = \emptyset$$
> Active allocations are guaranteed to have disjoint physical memory ranges.
>
> If allocation $A$ is deallocated and allocation $B$ is subsequently allocated, physical memory reuse may yield $base(A) == base(B)$. However, because $A \ne B$, their provenance domains remain strictly distinct and non-interchangeable.

---

## 7. Verifier Invariants & Canonical Drop Glue

### 7.1 `Write` Semantic Transition
`Write { dst, value }` is atomic with respect to MVIR ownership and lifecycle state:
$$\text{INITIALIZED}(T) \xrightarrow{\text{Write}} \text{INITIALIZED}(T)$$
Lowering emits:
1. Destruction of prior content: `@DropGlue_T(dst)` (if `needs_drop(T)`).
2. Direct store: `store value, dst`.

### 7.2 Canonical Drop Glue
- `DropInPlace { ptr, ty }` invokes `@DropGlue_ty(ptr)`.
- The destruction sequence (e.g. reverse declaration order of fields) is dictated by the **Mellis Language Specification** and synthesized in the compiler middle-end.
- The backend codegen lowers pre-synthesized drop glue; it never introspects LLVM structs to invent destruction order.

### 7.3 ZST Invariants
- For ZST allocations ($layout.size == 0$), `AllocationFact.is_zst == true`.
- `Dealloc { ptr, 0, align }` on a ZST fact is verified as a valid no-op. The verifier does not require physical heap block tracking for ZSTs.

---

## 8. MVIR to LLVM Lowering Summary

| MVIR Primitive | LLVM Backend Emission (`mellis-backend`) |
|:---|:---|
| `Alloc { site, size, align }` | `call ptr @__mellis_alloc(i64 size, i64 align)` |
| `Realloc { ptr, old_sz, old_al, new_sz }` | `call ptr @__mellis_realloc(ptr, i64 old_sz, i64 old_al, i64 new_sz)` |
| `Dealloc { ptr, size, align }` | `call void @__mellis_dealloc(ptr, i64 size, i64 align)` |
| `Initialize { dst, value }` | `store value, dst` (raw store into uninitialized memory) |
| `Write { dst, value }` | Call `@DropGlue_ty(dst)` (if `needs_drop`), then `store value, dst` |
| `MoveOut { src }` | `load src` into SSA register; marks place uninit in borrowck |
| `DropInPlace { ptr, ty }` | If `needs_drop(ty)`: `call void @DropGlue_ty(ptr)`. If `!needs_drop`: 0 instructions. |

---

## 9. Summary & Transition Criteria to P6

With this P5 Hardened Specification:
1. The **7 decomposed memory operations** are mathematically locked.
2. **`Realloc` is established as a first-class memory primitive** with compound effect tracking.
3. **Immutable `AllocationFact` is decoupled from path-sensitive `MemoryState`**, enabling CFG dataflow analysis.
4. **Static `AllocationSiteId` is distinguished from dynamic `AllocId`**.
5. **`MoveOut` is formally distinguished from `Load` and `RawLoad`**.
6. **`Write` is specified as a single ownership-state transaction**.
7. **Active allocation non-aliasing** is formalized.

---

## 10. The 7 Frozen MVIR Memory Invariants (MVIR-MEM-1 to MVIR-MEM-7)

These seven invariants represent the non-negotiable contract between the FrontEnd/Canonicalizer, Verifier, Optimizer, and Backend Code Generator:

### `MVIR-MEM-1`: Explicit Lifecycle Semantics
> **Every memory-affecting instruction has explicit lifecycle semantics.**
> There are no untyped or implicit memory transitions in canonical MVIR. Every allocation, resize, deallocation, initialization, assignment/overwrite, move, or destruction is expressed through one of the 7 primitive operations (`Alloc`, `Realloc`, `Dealloc`, `Initialize`, `Write`, `MoveOut`, `DropInPlace`).

### `MVIR-MEM-2`: Canonical Boundary Insulation
> **No legacy memory instruction reaches the Verifier or Backend.**
> Legacy composite instructions (`HeapAlloc`, `BoxNew`, `BoxFree`) are strictly lowering artefacts of earlier compiler stages. The `LegacyCanonicalizer` eliminates them at the entry boundary of MVIR optimization. The Verifier and LLVM Backend reject any IR containing non-canonical memory instructions.

### `MVIR-MEM-3`: Provenance & Non-Revocation
> **Every pointer use must have valid, non-revoked provenance.**
> Dereferencing or accessing storage via a pointer whose provenance fact has been invalidated (via `Dealloc` or `Realloc`) is an instant verifier failure. Allocation identities are deterministic, and temporal alias revocation is enforced across all CFG edges.

### `MVIR-MEM-4`: MoveOut Consumes Ownership
> **`MoveOut` transitions place lifecycle state $\text{Initialized} \to \text{Moved}$.**
> `MoveOut` is an effectful state transition (`MemoryEffect::Move`). It is distinct from `Load` (which is a read on `Copy` types leaving the place `Initialized`). Accessing a place after `MoveOut` via `Load`, `MoveOut`, or `DropInPlace` is strictly forbidden and rejected as a use-after-move or drop-after-move violation.

### `MVIR-MEM-5`: Single Drop Obligation
> **`DropInPlace` is valid exactly once for an initialized owned value.**
> `DropInPlace` requires the target place to be in the `Initialized` state. Calling `DropInPlace` on an `Uninitialized`, `Moved`, or already dropped place is an invariant violation (rejecting double-drop and drop-after-move). Upon execution, `DropInPlace` transitions the place to `Uninitialized`.

### `MVIR-MEM-6`: Clean Storage Deallocation
> **`Dealloc` is permitted only after the allocation is fully inactive.**
> Before `Dealloc` executes on an `AllocId`, all places and subfield projections within that allocation must be either `Uninitialized` or `Moved`. Attempting to deallocate storage containing live (`Initialized`) fields violates this invariant.

### `MVIR-MEM-7`: Optimization Preservation
> **Optimizations MUST preserve memory lifecycle and provenance semantics.**
> Dead Code Elimination (DCE) and transformation passes must respect `InstructionEffects`. Effectful memory operations (`Alloc`, `Realloc`, `Dealloc`, `Initialize`, `Write`, `MoveOut`, `DropInPlace`) and traps (`BoundsCheck`, `Div`, `Rem`) have observable side-effects and must NEVER be deleted simply because their SSA return values are unused.

---

## 11. Drop & Dealloc Trap/Abort Contract (Runtime ABI v1.0)

In the Luna Compiler v1.0 runtime architecture:
1. **No-Unwind Execution Model**:
   - The Luna v1.0 runtime does not support stack unwinding. Panics, bounds violations, and terminal allocator traps trigger immediate process termination (`@__mellis_abort` / trap).
2. **Destructor Trap Invariant**:
   - If a destructor invocation inside `@DropGlue_T` triggers a panic or trap:
     $$\text{DropInPlace} \xrightarrow{\text{Trap}} \text{Process Abort}$$
   - Execution terminates immediately. Downstream instructions—including the accompanying `Dealloc`—never execute.
   - Consequently, partially dropped allocations never enter an inconsistent deallocation path in the v1.0 runtime ABI.

---

## 12. Multi-Hop Realloc Provenance & AllocationLineage

When an allocation undergoes repeated reallocations across a chain:
$$p_0 = \text{Alloc}(A_0) \implies p_1 = \text{Realloc}(p_0, A_0 \to A_1) \implies p_2 = \text{Realloc}(p_1, A_1 \to A_2)$$

1. **Deterministic Lineage**:
   All allocations in the chain share the same persistent `AllocationLineage` identifying the logical growth of the buffer.
2. **Stepwise Revocation**:
   - Reallocating $A_0 \to A_1$ revokes $p_0$ and terminates $A_0$.
   - Reallocating $A_1 \to A_2$ revokes $p_1$ and terminates $A_1$.
3. **Validity State**:
   - $p_0$: **Revoked** $\implies$ any read/write/deref is rejected.
   - $p_1$: **Revoked** $\implies$ any read/write/deref is rejected.
   - $p_2$: **Valid** $\implies$ read/write/deref succeeds.

