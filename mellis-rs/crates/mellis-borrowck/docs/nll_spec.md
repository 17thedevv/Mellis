# Mellis Non-Lexical Lifetimes (NLL) Spec

This document describes the design and implementation of Non-Lexical Lifetimes (NLL) inside the Mellis compiler.

## Core Concepts

### 1. Loan Identity and Stability
The primary building block of NLL in Mellis is the `Loan`. In earlier lifetime implementations, loans were often represented by dynamically generated identifiers (e.g., an incrementing `usize`) which caused issues when propagating through control-flow loops. 

In Mellis, a **Loan's identity is directly mapped to the `ValueId`** of the `mellis-mvir` Instruction that originated the borrow (the `Borrow` instruction). This guarantees:
- **Stability**: Back-edges in a Control Flow Graph (CFG) do not generate infinite new loans.
- **Convergence**: A fixed-point iteration over the CFG will naturally converge in a finite number of steps because the universe of loans is strictly bounded by the number of `Borrow` instructions.

### 2. Forward Dataflow and Fixed-Point Iteration
The core region computation uses a forward dataflow analysis.

- **Abstract State**: The abstract state at any program point is a `BorrowStateData` structure containing a set of active `ValueId`s representing live loans.
- **Transfer Function**:
  - `Borrow(place)`: Generates a new loan (adds its own `ValueId` to the state).
  - **SSA Value Reuse**: In TVA, SSA reuse (e.g. `val2 = val1`) would cause lifetimes to be incorrectly truncated if analyzed based on syntactic scope. In this NLL spec, because the region computation iterates over actual `Instruction` flow, any operation that propagates a borrowed value keeps the original `Borrow`'s `ValueId` active in the `BorrowStateData` until all uses (including reused SSA variables) are strictly dominated by a kill or fall out of liveness. This specifically prevents the false-negative "premature drop" bug seen in Scenario 4b.
  - `Drop(place)` / Implicit Kills: Removes loans from the state when they are killed.
- **Merge Operator (Join)**: When control flow merges (e.g. `phi` blocks or after `if/else`), the incoming states are joined via Set Union.
- **Fixed-Point Iteration**: The `dataflow.rs` engine propagates these abstract states iteratively across the CFG blocks until no state changes. The stability of `Loan = ValueId` guarantees this terminates rapidly (in practice, less than ~32 iterations even on complex cyclic CFGs).

### 3. Call-Effect Integration
Mellis integrates NLL with the Interprocedural Call-Effect summary (`CallEffectSummary`):
- When a loan is active and the borrowed data is passed to a function call, the `Call-Effect` lattice evaluates the side-effects.
- **Read / ReadWrite**: A mutable/immutable borrow may be aliased or modified. If there is a conflict (e.g., passing a mutable loan to two conflicting arguments), it emits a diagnostic.
- **MayEscape**: If the called function is known to let a reference outlive the call (e.g. returning it or storing it into a global variable), the Call-Effect mechanism poisons the provenance via the `MayEscape` flag. Subsequent invalid usages trigger an aliasing or use-after-free error, correctly identifying the escape path.
- **FFI Boundary**: The C-ABI boundary is strictly treated via these effects, relying on correct C-ABI rules (`Read`/`ReadWrite`/`MayEscape` based on pointers) without needing ad-hoc "escape hatch" fallback rules.

## Correctness and Soundness
The fixed-point iteration engine combined with the stable identity provides a sound region computation scheme that:
1. Cannot diverge on cyclic control flows.
2. Integrates directly with interprocedural lattice summaries (avoiding the need for an external, unbounded lifetime constraint graph).
3. Safely handles FFI closures and callbacks without explicit lifetime annotations, by conservatively propagating `MayEscape`.
