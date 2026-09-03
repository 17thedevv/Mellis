# Non-Lexical Lifetimes (NLL) & Region Computation Specification

Mellis uses a Non-Lexical Lifetime (NLL) system based on the control-flow graph (CFG) of the Mellis Virtual Intermediate Representation (MVIR). This document serves as the formal specification for how regions are assigned, how liveness is computed, and how aliases interact.

## 1. Core Concepts

- **Region ($R$)**: A set of CFG points (instructions) where a specific borrow is "alive" (i.e., its value may be used in the future).
- **Borrow Origin**: An MVIR instruction that creates a reference (e.g., `BorrowShared`, `BorrowMut`, `AddressOf`).
- **Liveness ($L$)**: A region $R_i$ is live at point $P$ if there is a path from $P$ to a use of the reference associated with $R_i$, and the reference is not redefined along that path.

## 2. Liveness Dataflow Equations

Liveness is computed backwards over the MVIR CFG:

1. **GEN(n)**: The set of regions used at instruction $n$.
2. **KILL(n)**: The set of regions redefined (overwritten) at instruction $n$.

$$ LiveIn(n) = GEN(n) \cup (LiveOut(n) - KILL(n)) $$
$$ LiveOut(n) = \bigcup_{s \in successors(n)} LiveIn(s) $$

A region $R$ contains point $n$ iff $R \in LiveOut(n) \cup GEN(n)$.

## 3. The Subtlety of SSA Reuse vs. New Borrows

During the development of TVA (Taint Value Analysis), a critical subtlety in the borrow checker's region computation was exposed. It is imperative that all compiler passes (especially FFI/C-ABI integration) distinguish between these two scenarios.

### Scenario 1: SSA Value Reuse
```rust
%1 = borrow_shared %local_x  // Originates Region R1
%2 = call @foo(%1)           // R1 is live here
%3 = call @bar(%1)           // R1 is STILL live here. No new region is created.
```
**Mechanism:** Because MVIR uses Single Static Assignment (SSA) for operands, reusing `%1` implies reusing the *exact same borrow token*. The liveness of $R_1$ simply stretches to include the call to `@bar`. The compiler does *not* view this as two overlapping borrows; it is a single borrow used twice.

### Scenario 4b: New Borrow Creation
```rust
%1 = borrow_shared %local_x  // Originates Region R1
%2 = call @foo(%1)           // R1 is live here

%3 = borrow_shared %local_x  // Originates Region R2
%4 = call @bar(%3)           // R2 is live here
```
**Mechanism:** The `borrow_shared` instruction explicitly mints a *new* region. $R_1$ and $R_2$ are distinct regions. 
- If `%1` is never used after `%3`, $R_1$ is dead before $R_2$ starts.
- If `%1` *is* used after `%3`, $R_1$ and $R_2$ overlap. Because they are both shared borrows, the overlap is valid. However, if either was `borrow_mut`, the O/B core would flag an overlapping mutation error.

### Why the Distinction Matters for FFI/Taint

When a reference crosses a boundary (like an FFI call to C), the borrow checker must know exactly *which* regions are tainted.
- In **Scenario 1**, tainting `%1` at the FFI boundary means all subsequent uses of `%1` (like `%3 = call @bar(%1)`) are automatically operating on a tainted/unsafe region.
- In **Scenario 4b**, if `%1` is passed to C, only $R_1$ is tainted. The subsequent borrow `%3` creating $R_2$ is a fresh, safe borrow (assuming the FFI call didn't violate exclusivity), because the borrow checker evaluates the safety of $R_2$ independently.

Any future static analysis or C-ABI boundary logic must respect this fundamental difference between "extending a region's liveness via SSA reuse" and "minting a new region via explicit borrow".

## 4. Mutable Borrow Exclusivity

For any local variable $V$:
If a mutable region $R_{mut}$ originating from $V$ is live at point $P$, then **no other region** (shared or mutable) originating from $V$ can be live at point $P$.

$$ \forall P \in CFG, \forall R_i, R_j \text{ from } V : (R_i \text{ is Mut} \land R_i \in Live(P)) \implies R_j \notin Live(P) $$
