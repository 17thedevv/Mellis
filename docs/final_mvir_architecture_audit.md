# MVIR Architecture Audit Report: Actual Implementation vs. P0–P6 SSOT

- **Audited Target**: Mellis Compiler Intermediate Representation (`mellis-mvir`), Optimizer Verifier & DCE (`mellis-optimizer`), Borrowck Engine (`mellis-borrowck`), Binary IR Serialization (`mellis-mlib`), and LLVM Codegen (`mellis-backend`)
- **Authority**: Evaluated against Frozen RFCs: **P0 (Memory Model)**, **P1 (Layout Model)**, **P2 (Pointer Provenance)**, **P3 (Allocator Contract)**, **P4 (Runtime Memory ABI)**, **P5 (MVIR Operations)**, **P6 (Borrowck Ownership)**
- **Audit Date**: 2026-09-10

---

## 1. Executive Summary & Status Classification

An exhaustive codebase audit was conducted across all 15 crates in the workspace. While the fundamental structural foundations (SSA values, basic blocks, place projections, LLVM runtime function declarations) are well-architected, there is a significant semantic gap between the current implementation and the frozen P0–P6 memory specifications.

```text
┌────────────────────────────────────────────────────────┐
│                   Audit Results Summary                │
├────────────────────────────────────────┬───────────────┤
│ [CONFORMANT] Foundations               │ 3 Subsystems  │
│ [INCOMPLETE] Partial Implementations   │ 3 Subsystems  │
│ [CONTRADICTION] Semantic Contradictions│ 2 Subsystems  │
│ [BLOCKER] Architectural Blockers       │ 4 Subsystems  │
└────────────────────────────────────────┴───────────────┘
```

---

## 2. The 5 Critical Architectural Questions: Audit Answers

### Q1: Does MVIR truly differentiate raw storage from typed objects?
> **Answer: NO [ARCHITECTURAL BLOCKER]**
>
> In the current codebase:
> - The compiler only has `Instruction::Store { ptr, value }` and `Instruction::Load { ptr }`.
> - `Instruction::Alloc`, `Initialize`, and `Write` do not exist.
> - A store to uninitialized heap memory uses the identical instruction (`Store`) as an assignment to an active place, with zero drop-flag or destruction tracking.
> - The 4-state lifecycle machine (`ABSENT -> UNINITIALIZED -> INITIALIZED -> ABSENT`) is not represented in the IR.

### Q2: Is `Realloc` truly first-class, or is it treated as a generic `CallDirect`?
> **Answer: NO [ARCHITECTURAL BLOCKER]**
>
> In the current codebase:
> - `Instruction::Realloc` does not exist in `enum Instruction` in `crates/mellis-mvir/src/mvir.rs`.
> - Reallocation can only occur via an opaque `Instruction::CallDirect { callee: __mellis_realloc, args }`.
> - Neither `mellis-optimizer` nor `mellis-borrowck` recognizes reallocation as a memory or provenance event: old pointers are not invalidated, and active loans extending across reallocations are not checked.

### Q3: Does `AllocationFact` exist across CFG/SSA as specified in RFC P5?
> **Answer: NO [ARCHITECTURAL BLOCKER]**
>
> In the current codebase:
> - `AllocationFact`, `AllocationSiteId`, and path-sensitive `MemoryState` do not exist.
> - `Function` only stores `values: Vec<ValueData>` and `blocks: Vec<BasicBlock>`.
> - The verifier in `crates/mellis-optimizer/src/verifier.rs` only performs syntactic operand checks; it has zero knowledge of allocation sizes, alignments, ZST status, or deallocation pairing.

### Q4: Can the optimizer break object lifecycles (DCE / reordering)?
> **Answer: YES [SEMANTIC CONTRADICTION]**
>
> In `crates/mellis-optimizer/src/passes/dce.rs`:
> - DCE relies on a blunt boolean check at line 120:
>   ```rust
>   let has_side_effects = matches!(val.inst, Instruction::Store { .. } | Instruction::CallDirect { .. } ... | Instruction::HeapAlloc | Instruction::BoxNew { .. } | Instruction::BoxFree { .. } | Instruction::Drop { .. });
>   ```
> - There is no structured `InstructionEffects` (`MemoryEffect`, `ProvenanceEffect`, `DropEffect`).
> - The optimizer does not know that an `Alloc` carries an obligation paired with `Dealloc`, nor does it know that memory operations cannot be reordered across deallocation or reallocation.

### Q5: Do `Place` and `Value` have enough expressive power for field-sensitive move/borrow as required by P6?
> **Answer: PARTIAL [INCOMPLETE & CONTRADICTION]**
>
> In `crates/mellis-borrowck`:
> - `Place` in `src/place.rs` has `local: ValueId` and `projections: Vec<Projection>` (`Field(usize)`, `Deref`, `Index`) with `is_ancestor_of` and `is_descendant_of`. **This is a strong foundation.**
> - HOWEVER:
>   1. `MoveState` in `src/move_analysis.rs` uses a flat map `HashMap<Place, MoveState>`. The **Place State Tree** (derived summary state on parents) is not formalized.
>   2. Partial move under drop: `move_analysis.rs` does not distinguish `has_user_drop(T)` from `needs_drop(T)`. Normal aggregate types containing drop fields are either incorrectly blocked from partial moves or allowed without proper subfield drop tracking.
>   3. `overlaps(P1, P2)` is not formalized: borrow conflict checking only checks ancestor/descendant prefix matching, failing on complex projection trees or dynamic index overlap.

---

## 3. Comprehensive 10-Dimension Audit Matrix

| Dimension | Component | Current Implementation State | Compliance Status |
|:---|:---|:---|:---|
| **1. Value Model** | `mellis-mvir::ValueData` | SSA values and `ValueOrigin` exist; no `PointerValue` provenance association. | `[INCOMPLETE]` |
| **2. Instruction Model** | `mellis-mvir::Instruction` | Lacks `Alloc`, `Realloc`, `Dealloc`, `Initialize`, `Write`, `MoveOut`, `DropInPlace`. Legacy `BoxNew`, `BoxFree`, `HeapAlloc` still present. | `[BLOCKER]` |
| **3. Allocation Facts** | `mellis-mvir` / `optimizer` | `AllocationFact`, `AllocationSiteId`, `AllocId`, and path-sensitive `MemoryState` are completely absent. | `[BLOCKER]` |
| **4. Place & Lifecycle** | `mellis-borrowck::place` | `Place` & `Projection` exist; lacks Place State Tree, subobject drop flags, and `overlaps` relation. | `[INCOMPLETE]` |
| **5. Memory Effects** | `mellis-borrowck::effect` | Has `AccessKind`, `OwnershipKind`; lacks structured `InstructionEffects` (Memory, Provenance, Drop) in optimizer. | `[BLOCKER]` |
| **6. Verifier** | `mellis-optimizer::verifier` | Only checks operand index bounds; zero semantic memory/layout/lifecycle validation. | `[INCOMPLETE]` |
| **7. Optimizer** | `mellis-optimizer::dce` | Uses blunt `has_side_effects: bool` tripwire; no lifecycle or compound effect protection. | `[CONTRADICTION]` |
| **8. Binary IR** | `mellis-mlib::ir` | `MlibInstruction` still serializes `BoxNew`, `BoxFree`, `MarkInit`, `ListNew`; lacks the 7 primitives. | `[BLOCKER]` |
| **9. Backend Codegen** | `mellis-backend::llvm_codegen` | Declares `__mellis_alloc/dealloc/realloc`; `BoxNew` falls through to wildcard zero; `BoxFree` has crude hardcoded `(8, 8)` call; lacks lowering for 7 primitives. | `[BLOCKER]` |
| **10. Partial Moves & Drop** | `mellis-borrowck::move_analysis` | `needs_drop` and `has_user_drop` are conflated; no field-sensitive drop elaboration. | `[CONTRADICTION]` |

---

## 4. Itemized Deficiencies & Patch Requirements

### Deficiency 1: Legacy Monolithic Instructions in MVIR & Serialization
- **Files**: `crates/mellis-mvir/src/mvir.rs`, `crates/mellis-mlib/src/ir.rs`, `crates/mellis-mlib/src/writer.rs`, `crates/mellis-mlib/src/reader.rs`
- **Current State**: `BoxNew`, `BoxFree`, and untyped `HeapAlloc` are present.
- **Required Action**: Remove legacy variants. Add the 7 primitive memory instructions:
  - `Alloc { site: AllocationSiteId, size: Operand, align: Operand }`
  - `Realloc { ptr: Operand, old_size: Operand, old_align: Operand, new_size: Operand }`
  - `Dealloc { ptr: Operand, size: Operand, align: Operand }`
  - `Initialize { dst: Operand, value: Operand }`
  - `Write { dst: Operand, value: Operand }`
  - `MoveOut { src: Operand }`
  - `DropInPlace { ptr: Operand, ty: SemanticTypeId }`

### Deficiency 2: Missing Allocation Metadata (`AllocationFact`, `MemoryState`)
- **Files**: `crates/mellis-mvir/src/mvir.rs`, `crates/mellis-optimizer/src/verifier.rs`
- **Current State**: No allocation metadata exists.
- **Required Action**:
  - Define `AllocationSiteId(pub u32)`.
  - Define `AllocationFact { id: AllocId, site: AllocationSiteId, layout: Layout, is_zst: bool, domain: AllocatorDomain }`.
  - Define path-sensitive `MemoryState { allocations: BTreeMap<AllocId, AllocationState>, places: BTreeMap<PlaceId, PlaceLifecycleState> }`.
  - Upgrade `verifier.rs` to audit memory instructions against these facts.

### Deficiency 3: Optimizer Effect System
- **Files**: `crates/mellis-mvir/src/mvir.rs`, `crates/mellis-optimizer/src/passes/dce.rs`
- **Current State**: `dce.rs` uses `has_side_effects: bool`.
- **Required Action**: Define `InstructionEffects` with `MemoryEffect` (including `Reallocate` compound effect), `ProvenanceEffect`, and `DropEffect`. Update DCE to preserve paired allocation obligations.

### Deficiency 4: Backend Lowering Gap
- **Files**: `crates/mellis-backend/src/llvm_codegen.rs`
- **Current State**: `llvm_codegen.rs` does not lower the 7 primitives.
- **Required Action**:
  - Map `Alloc` $\implies$ `@__mellis_alloc(size, align)`.
  - Map `Realloc` $\implies$ `@__mellis_realloc(ptr, old_sz, old_al, new_sz)`.
  - Map `Dealloc` $\implies$ `@__mellis_dealloc(ptr, size, align)`.
  - Map `Initialize` $\implies$ LLVM `store value, dst`.
  - Map `Write` $\implies$ call `@DropGlue_T(dst)` (if `needs_drop`), then LLVM `store value, dst`.
  - Map `MoveOut` $\implies$ LLVM `load src`.
  - Map `DropInPlace` $\implies$ call `@DropGlue_T(ptr)`.

### Deficiency 5: Borrowck Place Tree & Partial Move Distinction
- **Files**: `crates/mellis-borrowck/src/move_analysis.rs`, `crates/mellis-borrowck/src/place.rs`
- **Current State**: `has_user_drop` is not separated from `needs_drop`. Flat `MoveState` map used.
- **Required Action**:
  - Separate `has_user_drop(T)` (`impl Drop for T`) from `needs_drop(T)` (`has_user_drop` OR subfield needs drop).
  - Enforce `E3002 PartialMoveUnderDrop` strictly when `has_user_drop(T) == true`.
  - Allow partial moves on normal aggregate types, tracking field survival via Place State Tree.
  - Implement formal `overlaps(P1, P2)` function.

---

## 5. Architectural Alignment Roadmap to P8 Freeze

```text
┌────────────────────────────────────────────────────────┐
│             Audit Findings: 4 Blockers Identified      │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 1: MVIR Core Primitives & Serialization Patch    │
│ - Introduce 7 primitive instructions                   │
│ - Deprecate BoxNew, BoxFree, HeapAlloc                 │
│ - Update .mlib ir, reader, and writer                  │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 2: AllocationFact, MemoryState & Verifier Patch  │
│ - Add AllocationFact & AllocationSiteId                │
│ - Implement path-sensitive MemoryState in verifier     │
│ - Upgrade DCE with structured InstructionEffects       │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 3: Backend Lowering Patch                        │
│ - Implement 1-to-1 lowering for 7 primitives           │
│ - Connect canonical drop glue calling                  │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 4: Borrowck Place State Tree & Overlaps Patch    │
│ - Formalize Place State Tree (derived parent states)   │
│ - Enforce has_user_drop vs needs_drop (E3002)          │
│ - Implement overlaps(P1, P2)                           │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Phase 5: Execute P7 Conformance Verification Matrix    │
│ - Run Suites A through L                               │
│ - Run Cross-Phase Semantic Trace                       │
│ - Run Optimization Equivalence K.4                     │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│                 P8: Tier 1 Final Freeze                │
└────────────────────────────────────────────────────────┘
```
