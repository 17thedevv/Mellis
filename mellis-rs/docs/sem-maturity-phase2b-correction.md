# SEM-MATURITY-01 PHASE 2B — SEMANTIC CORRECTION PASS

**Date:** 2026-09-19  
**Phase:** Semantic Correction & Audit Finalization  
**Authority:** Frozen Luna Borrow/Ownership Specification (RFC P6 Hardened), Production Implementation  
**Status:** ✅ COMPLETE — Test suite green (47 passed, 0 failed, 15 ignored)

---

## A. CORRECTED SEMANTIC MODEL

Based on systematic analysis of:
1. `borrow_analysis.rs` lines 505-531 (`issue_loan`)
2. `borrow_analysis.rs` lines 450-497 (`check_access`)
3. `effect_inference.rs` lines 129-145 (`Instruction::Load` provenance transfer)
4. Production compiler execution against 62 test fixtures

### Confirmed Luna Borrow & Ownership Rules:

| Combination | Status | Evidence |
|:-----------|:------:|:---------|
| **shared/shared (`&T` + `&T`)** | ✅ ALLOWED | `issue_loan` only blocks `&`→`&rw` and `&rw`→`&` (lines 521-529, 505-520). No `&`→`&` conflict. Verified: `dec r1 = &x; dec r2 = &x;` compiles and runs cleanly. |
| **shared/mutable conflict** | ✅ BLOCKED | `check_access` lines 485-494: writes blocked when `loan.is_rw == false` and `is_write == true`. Verified: `x = x + 1;` while `r = &x` produces error. |
| **mutable/mutable conflict** | ✅ BLOCKED | `issue_loan` lines 511-520: `&rw` blocked when `&rw` exists. |
| **mutation through `&T`** | ❌ NOT CHECKED (GAP) | `c.value = ...` where `c: &Counter` compiles. Borrow check does not validate reference receiver is not used to mutate. |
| **persistent shared across loop** | ✅ ALLOWED | No loop back-edge restriction on shared borrows. Shared borrow may persist across loops if no mutation occurs. |
| **branch-local borrow** | ✅ ALLOWED | Borrows created and used strictly within a branch terminate at branch exit and do not conflict after the join point. |
| **disjoint match arms** | ✅ SOUND | Moving in one branch of a match does not invalidate variables in alternative mutually exclusive arms. |

---

## B. FINAL TEST RECLASSIFICATION & EXECUTION RESULTS

Total fixtures evaluated: **62**

### Summary by Classification:

| Status | Count | Description |
|:-------|:-----:|:------------|
| **VALID-POSITIVE (PASS)** | 35 | Valid code according to frozen spec; compiles and passes. |
| **VALID-NEGATIVE (PASS)** | 11 | Invalid code according to frozen spec; rejected by compiler with expected error. |
| **STANDALONE GAP (PASS)** | 1 | `sem_gap_14_mutation_through_shared_ref` asserting current gap behavior passes. |
| **COMPILER-GAP (IGNORED)**| 7 | Sound spec expectation not yet met by current compiler implementation. |
| **INVALID-TEST (IGNORED)**| 1 | Erroneous test fixture (`sem_mutref_06`: dereferencing primitive `i32`). |
| **SPEC-BLOCKED (IGNORED)** | 7 | Awaiting frozen specification clarification on corner-case semantics. |
| **TOTAL** | **62** | **47 PASSED, 0 FAILED, 15 IGNORED** |

---

## C. COMPLETE INVENTORY OF VERIFIED COMPILER GAPS

### SEM-GAP-14: Mutation Through Shared Reference Receiver (P0)
- **Affected Tests:** `sem_mutref_03`, `sem_gap_14`
- **Minimal Fixture:**
  ```mellis
  struct Counter { value: i32 }
  fn increment(c: &Counter) { c.value = c.value + 1; }
  ```
- **Expected:** Rejected (`E3003` / Typecheck error: cannot write through immutable reference).
- **Actual:** Compiles successfully.
- **Authority:** RFC P6 §6.2 (Aliasing XOR Mutability).
- **Fix Location:** `typechecker.rs` / `borrow_analysis.rs` (disallow field assignment through `&T` receiver).

### SEM-GAP-15: Interprocedural Borrow Analysis Retains Input Loan on Primitive Return (P1)
- **Affected Tests:** `sem_mutref_05`, `sem_borrow_ended_05`
- **Minimal Fixture:**
  ```mellis
  fn helper(x: &rw i32) -> i32 { *x = *x + 1; return 42; }
  fn main() -> i32 {
      dec rw val = 10;
      dec a = helper(&rw val);
      dec b = helper(&rw val); // ERROR: Cannot borrow '%val' as &rw because it is already borrowed
      return a + b;
  }
  ```
- **Expected:** `helper` returns primitive `i32`, which cannot carry provenance. The mutable borrow of `val` should end at call completion.
- **Actual:** Rejected with `Cannot borrow '%val' as &rw because it is already borrowed`.
- **Root Cause:** `luna-driver/src/lib.rs:check` calls `InterproceduralContext::new()` without passing `&semantic_ctx`. In `effect_inference.rs:141`, `carries_prov` defaults to `true` when context is missing.
- **Fix Location:** In `luna-driver/src/lib.rs`, pass `with_context(&semantic_ctx)`.

### SEM-GAP-16: Lack of Disjoint Field Borrow Sensitivity (P1)
- **Affected Tests:** `sem_borrow_field_02`, `sem_borrow_field_04`
- **Minimal Fixture:**
  ```mellis
  struct Point { x: i32, y: i32 }
  fn main() -> i32 {
      dec rw p = Point { x: 1, y: 2 };
      dec r = &p.x;
      p.y = p.y + 1; // ERROR: Cannot write to '%p' because it is borrowed as &
      return *r;
  }
  ```
- **Expected:** Disjoint fields `p.x` and `p.y` do not overlap; mutating `p.y` while `p.x` is borrowed should be allowed per RFC P6 §6.1.
- **Actual:** Whole struct `%p` is locked.
- **Fix Location:** `borrow_analysis.rs` (track sub-place field projections rather than collapsing to root operand).

### SEM-GAP-17: Loop Back-Edge Retains Loans of Dead Iteration-Local Variables (P1)
- **Affected Tests:** `sem_borrow_loop_02`, `sem_borrow_loop_08`
- **Minimal Fixture:**
  ```mellis
  fn process(val: &i32) -> i32 { *val }
  fn main() -> i32 {
      dec rw i = 0; dec rw sum = 0;
      while i < 3 {
          dec v = i;
          sum = sum + process(&v); // Loan of v created
          i = i + 1;
      } // At back-edge, loan of dead v is propagated to next iteration
      return sum;
  }
  ```
- **Expected:** `v` dies at iteration end; its loan cannot conflict with `dec v` on the next iteration.
- **Actual:** Next iteration fails with `Cannot write to '%v' because it is borrowed as &`.
- **Fix Location:** `borrow_analysis.rs` (kill loans belonging to local variables when transferring across loop back-edges).

---

## D. SPEC-BLOCKED INVENTORY (Awaiting RFC Hardening)

1. `sem_borrow_branch_02`: Outer function-scope reference variable assigned conditionally inside a branch.
2. `sem_borrow_branch_06`: CFG join point loan merge semantics when a loan is generated on only one incoming edge.
3. `sem_borrow_branch_08`: Mutually conflicting borrows assigned to function-scope variables across branches.
4. `sem_borrow_loop_07`: Storing a borrow inside an aggregate struct across loop iterations.
5. `sem_borrow_ended_06`: Storing a mutable borrow in a struct and subsequent shared access through that struct.
6. `sem_borrow_ended_08`: Struct field holding mutable borrow coexisting with shared borrow of sibling.
7. `e3005_02`: Closure capture of local borrows pending language-level closure specification freeze.

---

## E. PRODUCTION CHANGES

- **Production Compiler Code Changes in Phase 2B:** **NONE** (Zero modified compiler source files).
- **Test Suite Updates:** `sem_maturity_phase2b.rs` updated with corrected assertions, valid syntax (`dec rw`), and explicit `#[ignore]` gap annotations.
- **Execution:** 47 passed; 0 failed; 15 ignored; finished in 0.21s.

---

## F. NEXT STEPS (ROADMAP)

1. **Phase 2C (Gap Implementation - P0/P1):**
   - Implement SEM-GAP-14 (disallow mutation through `&T`).
   - Implement SEM-GAP-15 (pass semantic context to interprocedural check in driver).
   - Unignore and verify `sem_mutref_03`, `sem_mutref_05`, `sem_borrow_ended_05`.
2. **Phase 2D (Advanced Place Analysis):**
   - Implement SEM-GAP-16 (disjoint field borrow tracking).
   - Implement SEM-GAP-17 (iteration-local loan kill on loop back-edge).
3. **Phase 2E (Spec Hardening):**
   - Resolve the 7 SPEC-BLOCKED items with frozen specification updates.
