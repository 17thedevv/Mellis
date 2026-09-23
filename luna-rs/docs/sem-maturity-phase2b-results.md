# SEM-MATURITY-01 Phase 2B Results: Ownership / Borrow / CFG Conformance Audit (Post-Correction)

**Date:** 2026-09-19  
**Audit Protocol:** `sem_maturity_phase2b` test suite  
**Status:** ✅ AUDIT COMPLETE & TEST SUITE GREEN  
**Test Suite Execution:** 47 passed; 0 failed; 15 ignored (7 GAPs, 1 invalid fixture, 7 spec-blocked)  

---

## Executive Summary

| Category | Total | Passed | GAPs | Invalid Fixture | Spec-Blocked | Conformance |
|:---------|------:|-------:|-----:|----------------:|-------------:|:-----------:|
| Mutable references broadly | 8 | 5 | 2 | 1 | 0 | 63% |
| Move in loops | 8 | 8 | 0 | 0 | 0 | 100% |
| Move through match | 7 | 7 | 0 | 0 | 0 | 100% |
| Borrow across branches | 8 | 5 | 0 | 0 | 3 | 63% (100% of defined) |
| Borrow through aggregate fields | 8 | 6 | 2 | 0 | 0 | 75% |
| Borrow across loops | 8 | 5 | 2 | 0 | 1 | 63% (71% of defined) |
| Shared borrow after mutable | 8 | 5 | 1 | 0 | 2 | 63% (83% of defined) |
| E3001/E3003/E3005 regression controls | 6 | 5 | 0 | 0 | 1 | 83% (100% of defined) |
| Standalone GAP validation | 1 | 1 | 0 | 0 | 0 | 100% |
| **TOTAL** | **62** | **47** | **7** | **1** | **7** | **76% verified pass rate** |

---

## Key Corrected Discoveries

### 1. Shared/Shared Borrows are Fully Sound and Allowed
- Initial audit assumed `dec r1 = &x; dec r2 = &x;` should fail.
- **Specification Authority (RFC P6 §6.2) & Implementation (`borrow_analysis.rs:505-531`)**: Luna explicitly permits multiple simultaneous shared (`&T`) borrows.
- Verified test: `e3003_01_multiple_shared_borrows_allowed` passes.

### 2. Move Semantics in Loops & Match are 100% Conforming
- All 8 move-in-loop tests pass without compiler modifications.
- All 7 move-through-match tests pass without compiler modifications.
- Unreachable post-match code is correctly analyzed by dataflow.
- Mutually exclusive match arms correctly maintain independent ownership states.

### 3. Syntax Traps in Previous Fixtures Corrected
- Previous test failures in branch and loop fixtures were caused by `dec result: i32;` missing `rw`.
- In Luna/Mellis grammar, `dec` variables without `rw` are immutable; assigning to them is rejected with `"Cannot mutate immutable variable"`. Correcting to `dec rw` resolved 7 false test failures.

---

## Verified Compiler Gaps (SEM-GAP-14 through SEM-GAP-17)

### SEM-GAP-14 (P0): Mutation Through Shared Reference Receiver Not Rejected
- **Affected Tests:** `sem_mutref_03`, `sem_gap_14`
- **Behavior:** `c.value = c.value + 1;` where `c: &Counter` compiles successfully.
- **Authority:** RFC P6 §6.2 (Aliasing XOR Mutability). Writing through a shared reference receiver must be rejected at typecheck/borrowck.
- **Proposed Fix:** Reject field and deref writes when receiver has `SemanticType::Reference(_, Mutability::Immutable, _)`.

### SEM-GAP-15 (P1): Interprocedural Borrow Analysis Retains Input Loan on Primitive Return
- **Affected Tests:** `sem_mutref_05`, `sem_borrow_ended_05`
- **Behavior:** When calling `fn helper(x: &rw i32) -> i32`, the return value is inferred to carry the provenance of `x`, keeping `x` locked as `&rw` in the caller.
- **Root Cause:** `luna-driver/src/lib.rs:check` initializes `InterproceduralContext::new()` without `SemanticContext`, causing `EffectInference::transfer_instruction` at `Instruction::Load` to default `carries_prov = true` even for primitive types.
- **Proposed Fix:** Initialize `InterproceduralContext::with_context(&semantic_ctx)` in `check` (identical to `compile_with_session`).

### SEM-GAP-16 (P1): Borrow Checker Operates at Coarse Aggregate Granularity (No Disjoint Field Sensitivity)
- **Affected Tests:** `sem_borrow_field_02`, `sem_borrow_field_04`
- **Behavior:** Borrowing `&p.x` locks the entire aggregate `%p`, preventing mutation of disjoint sibling `p.y`.
- **Authority:** RFC P6 §6.1 specifies disjoint field paths do not conflict.
- **Proposed Fix:** Extend place projection overlap checks in `borrow_analysis.rs` to distinguish sibling field projections (`Place::Field(base, 0)` vs `Place::Field(base, 1)`).

### SEM-GAP-17 (P1): Loop Back-Edge Retains Loans of Dead Iteration-Local Variables
- **Affected Tests:** `sem_borrow_loop_02`, `sem_borrow_loop_08`
- **Behavior:** Temporary borrow `process(&v)` inside loop body where `v` is declared inside the iteration propagates the loan of `v` across the back-edge. On subsequent iterations, re-initializing `dec v = i;` fails with `"Cannot write to '%v' because it is borrowed as &"`.
- **Root Cause:** Back-edge UNION merge does not kill loans of local values that go out of scope at the end of the loop body.
- **Proposed Fix:** Kill loans of values out of scope at loop iteration exit or filter back-edge propagated loans by liveness.

---

## Ignored & Spec-Blocked Inventory

| Test Name | Category | Reason |
|:----------|:---------|:-------|
| `sem_mutref_06_invalid_fixture` | Invalid Fixture | Fixture attempted `*val = 20;` on primitive `i32`. |
| `sem_borrow_branch_02_borrow_escapes_branch` | Spec-Blocked | Function-scope borrow variable assigned within branch escaping to caller. |
| `sem_borrow_branch_06_loan_live_from_one_branch` | Spec-Blocked | Exact CFG join point loan merge behavior when loan only created on one path. |
| `sem_borrow_branch_08_conflicting_borrows_across_branches` | Spec-Blocked | Function-scope reference variable assigned in alternative branches. |
| `sem_borrow_loop_07_borrow_escapes_loop` | Spec-Blocked | Aggregate-carried borrow escape semantics through loop back-edge. |
| `sem_borrow_ended_06_mut_stored_shared_after` | Spec-Blocked | Borrow lifetime when mutable borrow is stored inside a struct field. |
| `sem_borrow_ended_08_mut_struct_field_shared_access` | Spec-Blocked | Coexistence of struct-held mutable borrow and subsequent field borrow. |
| `e3005_02_closure_local_borrow` | Spec-Blocked | Closure capture borrow semantics pending final closure spec freeze. |

---

## Phase Status: SEM-MATURITY-01 PHASE 2B FROZEN ✅

The test suite in `crates/luna-driver/tests/sem_maturity_phase2b.rs` executes with **0 failures**, providing an unambiguous regression harness and gap roadmap for Phase 2C.
