# Phase 12: Closures — Complete Implementation Plan & Status

> **Document Purpose**: Standalone export for external audit. Contains Phase 12 plan, status of each sub-phase, and remaining work.

---

## Overview

Phase 12 implements **Closures** in the Mellis compiler. It is divided into 4 sub-phases:

| Sub-Phase | Description | Status |
|---|---|---|
| **12A** | Function Values & Indirect Calls | ✅ Complete |
| **12B** | Non-capturing Closures | ✅ Complete |
| **12C** | Capturing Closures (Environment generation, heap allocation, LLVM codegen) | ✅ Complete |
| **12D** | Borrow & Ownership Integration for Closures | 🔴 Not Started |

---

## 12A — Function Values & Indirect Calls (✅ Complete)

### Deliverables
- [x] **Semantic**: `SemanticType::Function { params, return_type }` — functions can be assigned to variables, passed as parameters, returned.
- [x] **MVIR**: `Instruction::CallDirect(FunctionId)` and `Instruction::CallIndirect(ValueId)` — distinct direct/indirect call paths.
- [x] **LLVM Backend**: `map_type` maps `SemanticType::Function` to LLVM pointer type. `build_indirect_call` constructs LLVM function types from semantic info and emits indirect call.
- [x] **Borrow Checker**: `move_analysis.rs` treats function types as trivially copyable.
- [x] **Tests**: `tests/indirect_call.ms` — covers assignment, params, return, mismatch, arity.

### Key Files Modified
- `crates/mellis-semantic/src/typechecker.rs` — function type unification
- `crates/mellis-mvir/src/mvir.rs` — `CallIndirect` instruction variant
- `crates/mellis-mvir/src/generator.rs` — lowering function values
- `crates/mellis-backend/src/llvm_codegen.rs` — `build_indirect_call`
- `crates/mellis-borrowck/src/move_analysis.rs` — function type copy semantics
- `crates/mellis-mlib/src/ir.rs`, `writer.rs`, `reader.rs` — MLib serialization for `CallIndirect`
- `crates/mellis-optimizer/src/verifier.rs` — verifier support

---

## 12B — Non-capturing Closures (✅ Complete)

### Deliverables
- [x] **Parser**: `|x: T| -> R { body }` parsed into `Expr::Closure`.
- [x] **Semantic**: Type inference for `Expr::Closure` — infers `fn(T) -> R`.
- [x] **Lowering**: Non-capturing closures lowered to plain function pointers (no environment).
- [x] **Tests**: `tests/closure_non_capture.ms` — closure syntax and HOF passing.

### Key Files Modified
- `crates/mellis-parser/src/parser.rs` — closure expression parsing
- `crates/mellis-semantic/src/resolver.rs` — `__closure_N` symbol generation
- `crates/mellis-semantic/src/typechecker.rs` — closure type inference
- `crates/mellis-mvir/src/generator.rs` — non-capturing closure lowering

---

## 12C — Capturing Closures (✅ Complete)

### Deliverables
- [x] **Semantic: Capture Analysis** — Typechecker detects free variables referenced across lambda boundaries and records them.
- [x] **Semantic: Environment Struct** — Generates anonymous `Tuple` struct type (`closure_env_types`, `closure_env_ptr_types` in `SemanticTables`).
- [x] **MVIR**: New instructions:
  - `Instruction::MakeClosure { func: GlobalId, env_ptr: Operand }` — creates fat pointer `{ code_ptr, env_ptr }`.
  - `Instruction::CallClosure { closure: Operand, args: Vec<Operand> }` — unpacks fat pointer and calls.
  - `Instruction::HeapAlloc` — heap-allocates environment struct (via `malloc`).
- [x] **MVIR Generator**: Lambda lowering:
  - Generates `HeapAlloc` for environment struct.
  - Populates environment fields with captured variables (by value).
  - Emits `MakeClosure` to create fat pointer.
  - Adds implicit `__env: ptr` first parameter to closure function body.
  - Replaces captured variable references with `FieldPtr` into `__env`.
- [x] **LLVM Backend**:
  - `map_type`: `SemanticType::Function` → `{ ptr, ptr }` (fat pointer ABI).
  - `MakeClosure`: Builds struct with `insert_value`.
  - `CallClosure`: Extracts `code_ptr` and `env_ptr` via `extract_value`, calls `code_ptr` with `env_ptr` as implicit first argument.
  - `HeapAlloc`: Emits `malloc` call with `sizeof` the environment struct.
- [x] **MLib Serialization**: `MakeClosure`, `CallClosure`, `HeapAlloc` all serialized/deserialized correctly in `ir.rs`, `writer.rs`, `reader.rs`.
- [x] **Optimizer**: Verifier and DCE updated for new instructions.
- [x] **Tests**: `tests/closure_capture.ms` — `make_adder` pattern, heap-allocated env, exit code 0.

### Key Design Decisions
1. **Fat Pointer ABI**: All `fn(...)` types are represented as `{ ptr code, ptr env }`. Normal functions have `env = null`.
2. **Heap Allocation**: Closure environments are `malloc`'d so they outlive the parent stack frame (e.g., `make_adder` returning a closure).
3. **Capture Mode**: Currently all captures are by-value (moved into env). Explicit `CaptureMode` enum (SharedBorrow, MutableBorrow, Move) is NOT yet implemented.

### Key Files Modified
- `crates/mellis-semantic/src/semantic_tables.rs` — `closure_env_types`, `closure_env_ptr_types`
- `crates/mellis-semantic/src/typechecker.rs` — env struct generation (lines ~770-790)
- `crates/mellis-semantic/src/resolver.rs` — `__closure_N` symbol (line ~439)
- `crates/mellis-mvir/src/mvir.rs` — `MakeClosure`, `CallClosure`, `HeapAlloc`
- `crates/mellis-mvir/src/generator.rs` — full lambda lowering with env population (~line 790-820)
- `crates/mellis-mvir/src/printer.rs` — pretty-printing
- `crates/mellis-backend/src/llvm_codegen.rs` — `MakeClosure` (~L630), `CallClosure` (~L598), `HeapAlloc` (~L475)
- `crates/mellis-mlib/src/ir.rs` — MLib instruction variants
- `crates/mellis-mlib/src/writer.rs` — serialization
- `crates/mellis-mlib/src/reader.rs` — deserialization
- `crates/mellis-optimizer/src/verifier.rs` — verification
- `crates/mellis-optimizer/src/passes/dce.rs` — dead code elimination
- `crates/mellis-borrowck/src/move_analysis.rs` — `MakeClosure` env_ptr handling
- `crates/mellis-borrowck/src/borrow_analysis.rs` — `CallClosure` borrow tracking
- `crates/mellis-borrowck/src/effect_inference.rs` — `CallClosure` effect inference

---

## 12D — Borrow & Ownership Integration (🔴 Not Started)

### Remaining Deliverables
- [ ] **Borrow Checker: Capture Action Registration** — `MakeClosure` should register a `Capture` action (SharedBorrow/MutableBorrow/Move) for each captured variable.
- [ ] **Borrow Checker: Reject Mutations to Captured Variables** — If a variable is borrowed by a closure, mutations to the original must be rejected while the closure is live.
- [ ] **Borrow Checker: Escaping Closure Lifetime Validation** — If a closure captures a reference (`&T`), the closure must not outlive the referent.
- [ ] **Borrow Checker: Recursive Drop** — Dropping a closure must recursively drop its captured environment.
- [ ] **CaptureMode Enum** — Implement explicit `CaptureMode { SharedBorrow, MutableBorrow, Move }` in semantic analysis to distinguish capture kinds.
- [ ] **Tests: Exhaustive Matrix** — Tests for:
  - Move capture: use-after-capture must be rejected.
  - Shared borrow capture: original can be read but not mutated.
  - Mutable borrow capture: original cannot be accessed.
  - Escaping lifetime: closure returning reference to local must be rejected.
  - Drop ordering: closure drop must drop env.
  - `closure_capture_move.ms` should produce a compile-time error (use of `x` after move into closure).

### Implementation Notes
- The infrastructure (`MakeClosure`, `CallClosure`, `HeapAlloc`, fat-pointer ABI) is fully in place from 12C.
- 12D is purely about **borrow checker integration** — no new MVIR instructions or LLVM codegen needed.
- The main work is in `crates/mellis-borrowck/src/move_analysis.rs` and `crates/mellis-borrowck/src/borrow_analysis.rs`.

---

## Test Files

| File | Phase | Purpose | Status |
|---|---|---|---|
| `tests/indirect_call.ms` | 12A | Function values, indirect calls | ✅ Passes |
| `tests/closure_non_capture.ms` | 12B | Non-capturing closure syntax | ✅ Passes |
| `tests/closure_capture.ms` | 12C | Capturing closure (make_adder) | ✅ Passes (exit 0) |
| `tests/closure_capture_move.ms` | 12D | Use-after-move diagnostic | 🔴 Should fail but doesn't yet |

---

## Architecture Diagram

```
Source Code (.ms)
    │
    ▼
┌─────────────────┐
│   Parser        │  Expr::Closure { params, body, captures }
└────────┬────────┘
         ▼
┌─────────────────┐
│   Resolver      │  __closure_N symbol, scope linking
└────────┬────────┘
         ▼
┌─────────────────┐
│  Typechecker    │  capture analysis, env struct generation,
│                 │  closure_env_types / closure_env_ptr_types
└────────┬────────┘
         ▼
┌─────────────────┐
│  MVIR Generator │  HeapAlloc → populate env → MakeClosure
│                 │  Closure body: __env param → FieldPtr reads
└────────┬────────┘
         ▼
┌─────────────────┐
│  Borrow Checker │  (12D: capture tracking, lifetime validation)
└────────┬────────┘
         ▼
┌─────────────────┐
│  LLVM Backend   │  fat pointer { code_ptr, env_ptr }
│                 │  CallClosure: extract + call with env
│                 │  HeapAlloc: malloc
└─────────────────┘
```
