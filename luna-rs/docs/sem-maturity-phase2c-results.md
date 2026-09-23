# SEM-MATURITY-01 Phase 2C Results: Mutability & Interprocedural Provenance Closure

**Date:** 2026-09-19  
**Branch:** `sem-maturity-01-phase2c`  
**Baseline Commit:** `d13ba24` (Phase 2A) -> `399b366` (hygiene) -> Phase 2C
**Status:** ✅ PHASE 2C COMPLETE, VERIFIED & FROZEN  
**Test Suite Execution:** 61 passed; 0 failed; 12 ignored  
**Total Fixtures in Suite:** 73 (62 Phase 2B fixtures + 11 Phase 2C Controls)  

---

## 1. Executive Summary

Phase 2C resolves three critical semantic maturity gaps while strictly preserving frozen specifications, language invariants, and the green baseline across the workspace:
1. **SEM-GAP-14 (P0) CLOSED**: Mutation through shared reference `&T` is now strictly rejected by place-based write capability enforcement in `luna-semantic`.
2. **SEM-GAP-15 (P1) CLOSED**: Interprocedural borrow analysis retaining input loan on primitive return is resolved and confirmed as an **INCOMPLETE ORIGINAL FIX of SEM-GAP-02** (`check()` omission in `luna-driver`).
3. **SEM-GAP-18 (P0 Soundness) CLOSED**: Interprocedural provenance loss across `Cast` and `PtrOffset` in `effect_inference.rs` is closed, bringing interprocedural effect inference into parity with intraprocedural `borrow_analysis.rs`.
4. **Architectural Hardening**: `InterproceduralContext::new(ctx: &'a SemanticContext)` now enforces providing `SemanticContext` at compile time, eliminating context-free effect inference regressions in production entry points.
5. **Heuristic Audit & Reversion**: Evaluated and rejected the speculative `fields.is_empty() => true` heuristic in `ty.rs`, proving it caused false-positive borrow rejections on zero-field structs. Pristine `ty.rs` is preserved.

---

## 2. SEM-GAP-14: Place-Based Write Capability Enforcement

### Problem & Invariant
- **Frozen Invariant**: `&T` is shared/immutable access; only `&rw T` permits mutable access (RFC P6 §6.2).
- **Previous Defect**: In `luna-semantic::TypeChecker::enforce_mutability`, `Expr::Member` and `Expr::TupleIndex` unconditionally recursed with `self.enforce_mutability(object)`. When `object` was a function parameter `c: &Counter`, `c` was declared with `SymbolKind::Variable`, so the check passed without checking whether the base reference was immutable (`&T`).

### Semantic Representation Audit
Inspected semantic types in `self.ctx.tables.expr_types`:
- For `c: &Counter`: `expr_types[c]` records `SemanticType::Reference(_, Mutability::Immutable, Counter)`.
- For `c.value`: `expr_types[c.value]` records `SemanticType::Primitive(BuiltinType::I32)`.
- Write capability is determined by inspecting the receiver's `SemanticType::Reference` mutability.

### Implementation in `crates/luna-semantic/src/typechecker.rs`
```rust
luna_ast::Expr::Member { object, .. } | luna_ast::Expr::TupleIndex { object, .. } => {
    let obj_ty = self.ctx.tables.expr_types.get(object).copied().unwrap_or(crate::ty::SemanticTypeId(0));
    match self.ctx.types.get(obj_ty) {
        SemanticType::Reference(_, crate::ty::Mutability::Immutable, _) => {
            let span = self.get_expr_span_for_diag(expr_id).unwrap_or(luna_common::Span::new(luna_common::ids::FileId(0), 0, 0));
            self.ctx.diagnostics.push(Diagnostic::error("E_CANNOT_MUTATE_IMMUTABLE_REFERENCE: Cannot mutate through an immutable reference `&T`")
                .with_span(span));
        }
        SemanticType::Reference(_, crate::ty::Mutability::Mutable, _) => {
            // Mutable reference provides write capability through dereference;
            // does not mutate the variable holding the reference.
        }
        _ => {
            self.enforce_mutability(object);
        }
    }
}
```
- **Scope**: Applies to `&T`, `&rw T`, member projections, nested member projections, tuple projections, indexing, and dereference.
- **Raw Pointers**: Intentionally unchanged; raw pointers follow their own dereference rules.
- **Diagnostic**: Reuses existing frozen `E_CANNOT_MUTATE_IMMUTABLE_REFERENCE`.

---

## 3. SEM-GAP-15: Driver Entry Point Parity (SEM-GAP-02 Completion)

### Root Cause Analysis: `B. INCOMPLETE ORIGINAL FIX`
- **Historical Evidence**: In commit `b660c5664fa4`, `compile_with_session()` was updated to pass `&semantic_ctx` via `InterproceduralContext::with_context()`. However, `check()` at `crates/luna-driver/src/lib.rs:245` was overlooked:
  ```rust
  let mut interproc = luna_borrowck::interprocedural::InterproceduralContext::new();
  ```
- **Mechanism**: In `crates/luna-borrowck/src/effect_inference.rs:138-143`, `Instruction::Load` checks `self.ctx`:
  ```rust
  let carries_prov = if let Some(ctx) = self.ctx {
      let sem_ty = self.func.values[val_id.0 as usize].ty;
      ctx.types.contains_reference(sem_ty)
  } else {
      true // fallback when ctx is None!
  };
  ```
  Because `check()` passed `ctx: None`, `carries_prov` defaulted to `true` for all loaded types, including primitives like `i32`.

### Architectural Hardening
1. `crates/luna-driver/src/lib.rs`: Updated `check()` to pass `&semantic_ctx`.
2. `crates/luna-borrowck/src/interprocedural.rs`: Changed `InterproceduralContext::new(ctx: &'a SemanticContext)` to require `&semantic_ctx`. Added `context_free_for_test()` exclusively for unit tests.
3. **Constructor Audit**: Grep confirmed **0** production call sites for `context_free_for_test()`. All production entry points pass `&semantic_ctx`.

---

## 4. SEM-GAP-18: Interprocedural Provenance across Cast and PtrOffset

### Problem & Discovery
- In intraprocedural analysis (`crates/luna-borrowck/src/borrow_analysis.rs:1375-1481`), `Instruction::Cast` and `Instruction::PtrOffset` properly propagated direct/carried taint and aliases.
- In interprocedural analysis (`crates/luna-borrowck/src/effect_inference.rs:392`), `Instruction::Cast` and `Instruction::PtrOffset` were grouped with `SizeOf`/`AlignOf` as no-ops.
- This silently dropped interprocedural return effects when a function cast a reference to a pointer (`r as *rw i32`) or indexed into an array (`&rw arr[0]`), allowing concurrent incompatible mutable borrows in callers.

### Implementation in `crates/luna-borrowck/src/effect_inference.rs`
```rust
Instruction::Cast { value, .. } => {
    self.add_direct_taint(state, val_id, value);
    self.add_carried_taint(state, val_id, value);
    if let Operand::Value(b) = value {
        state.aliases.insert(val_id, Operand::Value(*b));
    }
}
Instruction::PtrOffset { ptr, .. } => {
    self.add_direct_taint(state, val_id, ptr);
    self.add_carried_taint(state, val_id, ptr);
    if let Operand::Value(b) = ptr {
        state.aliases.insert(val_id, Operand::Value(*b));
    }
}
```
- Verified with Control 10 (`test_ctrl_cast_propagates_provenance`) and Control 11 (`test_ctrl_ptroffset_propagates_provenance`). Reverting either change causes immediate test failure.

---

## 5. Audit of `contains_reference` & Source/.llib Parity

1. **Heuristic Evaluation**:
   - The proposed heuristic `if fields.is_empty() { true }` in `crates/luna-semantic/src/ty.rs` was audited.
   - Demonstrated with `test_ctrl_zero_field_struct_no_provenance`: with `fields.is_empty() => true`, returning a reference-free zero-field struct (`struct Marker {}`) caused the compiler to falsely claim the container was still borrowed, rejecting subsequent valid writes.
   - Proved that `.llib` AST interfaces for standard iterators (`RawTableIter`) already contain pointer fields (`*u8`, `*K`, `*V`) and do not have empty fields.
   - **Decision**: Completely reverted `ty.rs` to clean baseline.

---

## 6. Test Suite Accounting & Conformance Verification

| Category | Count | Status | Notes |
|:---------|:------|:-------|:------|
| **VALID-POSITIVE** | 42 | ✅ PASS | 35 Phase 2B + 4 unignored (`sem_mutref_01, 02, 05`, `sem_borrow_ended_05`) + Controls 3, 4, 5, 8, 9 |
| **VALID-NEGATIVE** | 19 | ✅ PASS | 11 Phase 2B + 1 promoted (`sem_mutref_03`) + 1 closed (`sem_gap_14`) + Controls 1, 2, 6, 7, 10, 11 |
| **BUG CHARACTERIZATION** | 0 | - | All characterizations eliminated / promoted to conformance |
| **OPEN-COMPILER-GAP** | 3 | ⏸️ IGNORED | SEM-GAP-16 (2) & SEM-GAP-17 (1), deferred to Phase 2D |
| **INVALID-TEST** | 1 | ⏸️ IGNORED | `sem_mutref_06_invalid_fixture` (mutates immutable variable `r`) |
| **SPEC-BLOCKED** | 8 | ⏸️ IGNORED | `branch_02, 06, 08`, `loop_07, 08` (requires CFG/liveness fixed-point, SEM-GAP-17), `borrow_ended_06, 08`, `e3005_02` |
| **TOTAL PASSED** | **61** | ✅ GREEN | All active tests pass with 0 failures |
| **TOTAL IGNORED** | **12** | ⏸️ FROZEN | Strictly documented gaps and spec-blocked tests |
| **TOTAL FIXTURES** | **73** | - | 62 original + 11 controls |

---

## 7. Controls Verification Summary

1. `test_ctrl_gap14_direct_deref_write_rejected` (INVALID: `*r = 10` for `r: &i32`): **PASS**
2. `test_ctrl_gap14_nested_projection_rejected` (INVALID: `o.inner.y = 10` for `o: &Outer`): **PASS**
3. `test_ctrl_gap14_nested_projection_mutref_valid` (VALID: `o.inner.y = 10` for `o: &rw Outer`): **PASS**
4. `test_ctrl_gap14_owned_rw_mutation_valid` (VALID: `o.inner.y = 10` for `dec rw o = ...`): **PASS**
5. `test_ctrl_gap14_shared_read_valid` (VALID: `dec y = o.inner.y` for `o: &Outer`): **PASS**
6. `test_ctrl_prov_returned_mutref_retains_loan` (INVALID: returned `&rw` retains loan in caller): **PASS**
7. `test_ctrl_prov_aggregate_carries_provenance` (INVALID: aggregate carrying ref retains loan): **PASS**
8. `test_ctrl_prov_check_compile_agreement` (VALID: `check()` and `compile()` produce identical success): **PASS**
9. `test_ctrl_zero_field_struct_no_provenance` (VALID: zero-field struct does not falsely carry loan): **PASS**
10. `test_ctrl_cast_propagates_provenance` (INVALID: `Cast` retains borrow provenance across boundary): **PASS**
11. `test_ctrl_ptroffset_propagates_provenance` (INVALID: `PtrOffset` retains borrow provenance across boundary): **PASS**

---

## 8. Workspace Regression Verification

- `cargo test -p luna-semantic -p luna-borrowck`: **PASS** (19 unit, 10 integration suites).
- `cargo test --test stdlib_collection_iterators_acceptance_tests`: **PASS** (all 8 tests green).
- `cargo test --workspace -j 2 -- --test-threads=1`: **PASS** (all workspace suites green).
- `cargo test --workspace -j 2`: **PASS** (parallel execution green, zero flakes, zero races).
