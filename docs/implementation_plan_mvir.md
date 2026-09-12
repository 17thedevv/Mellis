# Implementation Plan: MVIR Architecture & Memory Lifecycle Resolution (P0–P6 SSOT v3)

Resolve the 3 architectural blockers and 4 refinements identified in the reviewer audit to achieve complete, mathematically sound conformance across `luna-mvir`, `luna-optimizer`, `luna-borrowck`, and `luna-backend`.

---

## Direct Resolution of Reviewer Critique (v3)

### 🔴 1. Crate Layering: Zero Backward Dependency
- **Problem**: `luna-mvir` must never depend on `luna-borrowck`.
- **Resolution**:
  - Relocate `Place` and `Projection` into `luna-mvir::place` (since `Place` only wraps `ValueId` and `Projection`).
  - `luna-mvir` owns the shared IR representation of memory places.
  - `luna-borrowck` re-exports and builds advanced ownership dataflow algorithms on top of `luna_mvir::place::Place`.
  - Crate flow remains strictly unidirectional:
    $$\text{luna-common} \longrightarrow \text{luna-ast} \longrightarrow \text{luna-mvir} \longrightarrow \text{luna-optimizer} \ \big| \ \text{luna-borrowck} \longrightarrow \text{luna-backend}$$

### 🔴 2. Decouple Type Identity in Effects (`MvirTypeId`)
- **Problem**: `DropEffect::InvokesDrop(SemanticTypeId)` leaks `luna-semantic` into MVIR effect queries.
- **Resolution**:
  - In `luna-mvir`, define:
    ```rust
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MvirTypeId(pub u32);
    ```
  - `DropEffect` becomes:
    ```rust
    pub enum DropEffect {
        None,
        InvokesDrop(MvirTypeId),
    }
    ```
  - `luna-mvir` does not depend on `luna-semantic` for its effect interface.

### 🔴 3. Persistent `AllocId` on Instructions
- **Problem**: If the verifier generates `AllocId`, IR passes and transformations cannot maintain stable allocation identities.
- **Resolution**:
  - `AllocId` and `AllocationLineage` are explicit IR fields on memory instructions:
    ```rust
    Instruction::Alloc {
        alloc_id: AllocId,
        lineage: AllocationLineage,
        site: AllocationSiteId,
        size: Operand,
        align: Operand,
    }

    Instruction::Realloc {
        old_alloc: AllocId,
        new_alloc: AllocId,
        ptr: Operand,
        old_size: Operand,
        old_align: Operand,
        new_size: Operand,
    }

    Instruction::Dealloc {
        alloc_id: AllocId,
        ptr: Operand,
        size: Operand,
        align: Operand,
    }
    ```
  - `AllocId` is an IR identity; the verifier verifies invariants but never invents identities.

### 🟠 4. Rigorous Lifecycle State Transitions
- **Leaf States**:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum PlaceLifecycleState {
      Uninitialized,
      Initialized,
      Moved,
  }
  ```
- **Transition Table**:
  - `Alloc`: Place $\rightarrow$ `Uninitialized`
  - `Initialize`: `Uninitialized` | `Moved` $\rightarrow$ `Initialized`
  - `Write`: `Initialized` $\rightarrow$ `Initialized` (precondition: MUST be `Initialized`; otherwise verifier error)
  - `MoveOut`: `Initialized` $\rightarrow$ `Moved`
  - `DropInPlace`: `Initialized` $\rightarrow$ `Uninitialized`

### 🟠 5. `PartiallyMoved` as Derived Query
- `PartiallyMoved` is **NOT** an enum variant in `PlaceLifecycleState`.
- Derived on demand via query method:
  ```rust
  pub enum AggregateState {
      Uninitialized,
      Initialized,
      Moved,
      PartiallyMoved,
  }

  impl MemoryState {
      pub fn aggregate_state(&self, place: &Place, children: &[Place]) -> AggregateState { ... }
  }
  ```

### 🟠 6. Verifiable Alias Revocation via `PointerProvenanceFact`
- `MemoryState` explicitly tracks pointer validity:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct PointerProvenanceFact {
      pub allocation: AllocId,
      pub valid: bool,
  }

  pub struct MemoryState {
      pub allocations: std::collections::BTreeMap<AllocId, AllocationState>,
      pub places: std::collections::BTreeMap<Place, PlaceLifecycleState>,
      pub pointers: std::collections::BTreeMap<ValueId, PointerProvenanceFact>,
  }
  ```
- When `Realloc { old_alloc, new_alloc, .. }` occurs:
  - For all pointers where `fact.allocation == old_alloc`: set `fact.valid = false`.
  - Any subsequent read, write, or dereference through a pointer with `valid == false` is caught and rejected by the verifier.

### 🟠 7. Clean Backend Separation & Drop Elaboration
- The backend is a pure machine code emitter and **never** calls `needs_drop(SemanticType)`.
- Drop obligations are elaborated upstream:
  - Middle-end emits explicit `DropInPlace` before reassignment/scope-exit.
  - Backend strictly emits:
    - `DropInPlace` $\implies$ call `@DropGlue_T`.
    - `Initialize` / `Write` $\implies$ LLVM `store`.
    - `MoveOut` $\implies$ LLVM `load`.

### 🟠 8. Explicit `LegacyCanonicalizer` Pass
- Create a dedicated `LegacyCanonicalizer` pass at the entrance of the optimization/verification pipeline:
  - Lowers `BoxNew(v)` $\rightarrow$ `Alloc` + `Initialize(v)`.
  - Lowers `BoxFree(p)` $\rightarrow$ `DropInPlace(p)` + `Dealloc(p)`.
- Downstream Verifier, Optimizer, Borrowck, and Backend **only** ever see the 7 canonical primitives.

---

## Proposed Changes

### Component 1: `luna-mvir` — Core Types & Shared `Place`

#### [MODIFY] [mvir.rs](file:///d:/fdlang/mellis-rs/crates/luna-mvir/src/mvir.rs)
- Define `AllocId(pub u32)`, `AllocationLineage(pub u32)`, `MvirTypeId(pub u32)`.
- Add explicit `alloc_id`, `lineage` fields to `Instruction::Alloc`, `Realloc`, `Dealloc`.
- Define `AllocationFact`, `AllocationState { Active, Terminated }`.
- Define `PlaceLifecycleState { Uninitialized, Initialized, Moved }`.
- Define `PointerProvenanceFact { allocation: AllocId, valid: bool }`.
- Define `MemoryState`.
- Define `InstructionEffects`:
  ```rust
  pub struct InstructionEffects {
      pub memory: MemoryEffect,
      pub provenance: ProvenanceEffect,
      pub drop: DropEffect,
      pub control_flow: ControlFlowEffect,
      pub may_trap: bool,
  }

  impl InstructionEffects {
      pub fn has_side_effects(&self) -> bool {
          self.may_trap
              || !matches!(self.memory, MemoryEffect::None)
              || !matches!(self.drop, DropEffect::None)
              || !matches!(self.control_flow, ControlFlowEffect::None)
      }
  }
  ```

#### [NEW] [place.rs](file:///d:/fdlang/mellis-rs/crates/luna-mvir/src/place.rs)
- Relocate `Place` and `Projection` from `luna-borrowck` into `luna-mvir::place`.

#### [MODIFY] [lib.rs](file:///d:/fdlang/mellis-rs/crates/luna-mvir/src/lib.rs)
- Export `pub mod place;` and re-export `Place`, `Projection`.

---

### Component 2: `luna-borrowck` — Use Shared `Place` & RFC P6 Rules

#### [MODIFY] [place.rs](file:///d:/fdlang/mellis-rs/crates/luna-borrowck/src/place.rs)
- Re-export `pub use luna_mvir::place::{Place, Projection};`.
- Preserve any borrowck-specific extension traits/methods.

#### [MODIFY] [move_analysis.rs](file:///d:/fdlang/mellis-rs/crates/luna-borrowck/src/move_analysis.rs)
- Implement RFC P6 Section 5.2:
  - `MOVE-PARTIAL-1`: If `has_user_drop(T)`, reject subfield move (`E3002`).
  - `MOVE-PARTIAL-2`: If `needs_drop(T) && !has_user_drop(T)`, allow subfield move.

---

### Component 3: `luna-optimizer` — Verifier, Canonicalizer & DCE

#### [NEW] [canonicalize.rs](file:///d:/fdlang/mellis-rs/crates/luna-optimizer/src/passes/canonicalize.rs)
- Implement `LegacyCanonicalizer` pass: lowers `BoxNew`/`BoxFree`/`HeapAlloc` to canonical primitives.

#### [MODIFY] [verifier.rs](file:///d:/fdlang/mellis-rs/crates/luna-optimizer/src/verifier.rs)
- Implement CFG dataflow verification with `MemoryState`:
  - Enforce `Write` precondition: `state(dst) == Initialized`.
  - Enforce `Dealloc` invariant: all places in `alloc_id` must be `Uninitialized` / `Moved`.
  - Enforce pointer validity via `PointerProvenanceFact`.

#### [MODIFY] [dce.rs](file:///d:/fdlang/mellis-rs/crates/luna-optimizer/src/passes/dce.rs)
- Use `inst.effects().has_side_effects()`.
- Never eliminate instructions where `may_trap == true`.
- Interprocedural / escape-aware reasoning for allocations: unescaped allocations with zero observable effects can be cleanly eliminated.

---

### Component 4: `luna-backend` — Pure Machine Lowering

#### [MODIFY] [llvm_codegen.rs](file:///d:/fdlang/mellis-rs/crates/luna-backend/src/llvm_codegen.rs)
- Pure machine lowering:
  - `Alloc` $\implies$ `@__mellis_alloc(size, align)`
  - `Realloc` $\implies$ `@__mellis_realloc(ptr, old_size, old_align, new_size)`
  - `Dealloc` $\implies$ `@__mellis_dealloc(ptr, size, align)`
  - `Initialize` / `Write` $\implies$ LLVM `store`
  - `MoveOut` $\implies$ LLVM `load`
  - `DropInPlace` $\implies$ call `@DropGlue_T`

---

## Verification Plan

### 1. Semantic Preservation Tests (End-to-End Positive)
- `test_alloc_init_move_dealloc`: Full lifecycle execution.
- `test_alloc_write_sequence`: Successive writes with drop obligations.
- `test_alloc_realloc_preservation`: Reallocation preserving prefix data.
- `test_partial_move_aggregate`: RFC P6 MOVE-PARTIAL-2 conformance.

### 2. Optimizer Preservation Tests
- Assert optimizer does NOT alter:
  - Active allocation count.
  - Required deallocation count.
  - Drop glue invocation sequence.
  - Trap behavior (`BoundsCheck`, `DivZero` preserved).

### 3. Verifier Boundary Tests (Corrupted MVIR Rejection)
- `test_reject_write_to_uninitialized`: Verifier rejects `Write` on uninitialized place.
- `test_reject_dealloc_with_live_field`: Verifier rejects `Dealloc` when a field is `Initialized`.
- `test_reject_deref_after_realloc`: Verifier rejects pointer dereference after `Realloc`.
- `test_reject_partial_move_under_user_drop`: Borrowck rejects partial move on `impl Drop` (`E3002`).

### 4. Full Workspace Regression
```powershell
cargo test --workspace
```
Must pass 100% across all crates without regression.
