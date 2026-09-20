# SPEC-HARDENING-01: Borrow, Path-Join, and Closure Non-Lexical Semantic Rules

**Status:** FROZEN & NORMATIVE  
**Authority:** Luna Semantic Core, RFC P6 (Hardened), NLL Dataflow Specification  
**Resolution Scope:** Closes language specification debt for Branch/Join, Loop Escape, Carried References, and Closure Captures.

---

## 1. Executive Summary & Problem Formulation

During Phase 2B/2C of the semantic maturity program, 8 conformance fixtures were placed under `SPEC-BLOCKED` status. These fixtures tested language corner cases where the static semantics at the intersection of non-lexical lifetimes (NLL), control-flow graph (CFG) joins, aggregate-carried provenance, and closure captures required formal language-level specification rather than ad-hoc compiler inferences.

This specification document freezes the normative language contracts for all 8 items across 4 semantic groups:
- **Group A (Branch / Join Semantics)**: `branch_02`, `branch_06`, `branch_08`
- **Group B (Loop / Path-Dependent Escape Semantics)**: `loop_07`, `loop_08`
- **Group C (Stored / Carried Reference Semantics)**: `ended_06`, `ended_08`
- **Group D (Closure Capture / Escape Semantics)**: `e3005_02`

---

## 2. Group A: Branch & Join Point Semantics

### RULE-BRANCH-JOIN-01 (`SEM-BRANCH-02`): Scope-Bounded Provenance Join & Escape
- **Normative Contract:**  
  At a CFG control-flow join point, the reference provenance of each live carrier represents the conservative join of all reachable incoming paths.
  A loan remains active after the join if and only if some live post-join carrier retains that loan's provenance, or the loan has otherwise escaped.
  
  If any reachable incoming path creates a loan whose referent belongs to an inner scope that expires at or before the join point, and a carrier live past the join point retains that loan, the program is statically rejected with:
  `error[E3005]: LocalBorrowEscape: Reference to local variable escapes block scope`
- **Valid Counterpart (Outer Referent):**  
  Where all incoming paths assign references to places outliving the join point, the carrier safely holds the conservative join of provenances, expiring when the carrier is no longer used.

### RULE-BRANCH-JOIN-02 (`SEM-BRANCH-06`): Path-Sensitive Asymmetric Loan Termination
- **Normative Contract:**  
  When a loan $L = \text{Loan}(p, \text{mode})$ is created along only one branch and assigned to a post-join carrier $c$:
  1. $L$ remains active after the join point as long as $c$ remains live.
  2. Any access to place $p$ conflicting with $L$'s borrow mode while $c$ is live is rejected with `error[E3003]: BorrowConflict`.
  3. Once all uses of $c$ terminate (carrier is dead), loan $L$ expires, and place $p$ is immediately released for unrestricted access.

### RULE-BRANCH-JOIN-03 (`SEM-BRANCH-08`): Multi-Branch Carrier Provenance Join & Exclusivity
- **Normative Contract:**  
  Control-flow branches of an `if/else` or `match` statement are mutually exclusive execution paths.
  1. Independent loans created in alternative branches are path-isolated: two borrows that would conflict if concurrent may validly exist in alternative branches if their carrier lifetimes do not conflict.
  2. When alternative branches assign distinct loans into the same live post-join carrier $c$ (e.g., `if cond { r = &rw x; } else { r = &rw y; }`), $c$ retains the joined provenance $\{x, y\}$ across the join point.
  3. While $c$ remains live, any access conflicting with $c$'s borrow mode on *either* referent ($x$ or $y$) is strictly rejected with `error[E3003]: BorrowConflict`.
  4. Loans represented solely by that carrier expire when the carrier dies, provided their provenance has not been transferred to another live carrier or otherwise escaped.

---

## 3. Group B: Loop & Path-Dependent Escape Semantics

### RULE-LOOP-ESCAPE-01 (`SEM-LOOP-07`): Dynamic Lifetime-Region Boundary Invariant
- **Normative Contract:**  
  A borrow whose referent belongs to an iteration-local lifetime must not escape across any edge that outlives that dynamic lifetime instance.
  1. The loop back-edge (continue/re-entry) is one such boundary; other boundaries include `break`, normal loop termination, return, or exit from an enclosing nested region.
  2. If an iteration-local variable's provenance is stored in an aggregate or variable that crosses any boundary outliving that iteration instance, the program is statically rejected with:
     `error[E3005]: LocalBorrowEscape: Reference to iteration-local variable escapes loop iteration`
  3. Conversely, updating an aggregate to borrow an outer variable that outlives the loop remains valid across iterations, provided no conflicting access to the referent occurs while the carrier is live.

### RULE-LOOP-PATH-02 (`SEM-LOOP-08`): Conditional Iteration-Local Loan Kill
- **Normative Contract:**  
  A borrow created conditionally within a loop iteration whose referent is iteration-local terminates upon completion of that iteration:
  1. The loan is killed at the iteration exit boundary and does not propagate across the loop back-edge to subsequent iterations.
  2. Subsequent iterations may freely define, mutate, and borrow new iteration-local instances without conflict with prior iterations.

---

## 4. Group C: Stored & Carried Reference Semantics

### RULE-CARRIED-REF-01 (`SEM-ENDED-06`): Multi-Carrier Aggregate Loan Expiration & Transfer
- **Normative Contract:**  
  An aggregate-carried loan expires only when no live carrier retains its provenance and the provenance has not escaped.
  1. While any live carrier retains provenance for an exclusive loan $\text{Loan}(x)$, conflicting access to $x$ is rejected with `error[E3003]: BorrowConflict`.
  2. If an aggregate $A$ carrying $\text{Loan}(x)$ transfers, moves, or projects its carried reference into another aggregate or variable $B$, the death or dropping of $A$ does *not* terminate $\text{Loan}(x)$ if $B$ remains live.
  3. Only when all live carriers ($A$, $B$, or subsequent aliases) retaining provenance for $\text{Loan}(x)$ have completed their last use does the carried loan terminate, restoring unrestricted access to $x$.

### RULE-CARRIED-REF-02 (`SEM-ENDED-08`): Carrier Storage vs Referent Identity & Disjoint Field Access
- **Normative Contract:**  
  A reference stored in a struct field (`holder.r = &rw x`) establishes `holder.r` as carrier storage for provenance over the distinct referent place $x$. Carrier field identity is distinct from referent identity:
  1. Storing a reference into `holder.r` carries provenance for referent $x$, but does not by itself borrow unrelated sibling storage fields (`holder.other`).
  2. Definitely-disjoint sibling places remain independently accessible and mutable, subject only to any loans they themselves carry.
  3. Whole-aggregate accesses (`holder`) continue to obey strict place-overlap rules: accessing the whole struct overlaps all of its fields and remains subject to aggregate loan conflict checks.
  4. The loan carried through `holder.r` expires only when no live carrier retains that provenance and the provenance has not escaped. The death of `holder` alone is insufficient if its reference provenance has been transferred to another live carrier (e.g., `q = holder.r`).

---

## 5. Group D: Closure Reference Capture & Escape Semantics

### RULE-CLOSURE-ESCAPE-01 (`E3005-02`): Closure Referent Lifetime Bounding
- **Normative Contract:**  
  A closure capturing variables by reference (`CaptureMode::SharedBorrow` or `CaptureMode::MutableBorrow`) must not outlive the captured referent's lifetime:
  1. Attempting to return a closure that captures a local borrow escapes past the termination of that local referent's lifetime, and is strictly rejected with:
     `error[E3005]: LocalBorrowEscape: Cannot return a closure that captures a local borrow`
  2. Local invocation of the closure within the valid lifetime of all captured referents is fully sound and accepted.

---

## 6. Conformance Matrix

| Fixture | Frozen Rule ID | Status | Expected Behavior |
| :--- | :---: | :---: | :--- |
| `sem_borrow_branch_02_borrow_escapes_branch` | `RULE-BRANCH-JOIN-01` | **ACTIVE CONFORMANCE** | Negative: inner local escaping branch rejected (`E3005`); Positive: outer referent valid. |
| `sem_borrow_branch_06_loan_live_from_one_branch` | `RULE-BRANCH-JOIN-02` | **ACTIVE CONFORMANCE** | Negative: mutation of referent while carrier live after join rejected (`E3003`); Positive: use after carrier dead accepted. |
| `sem_borrow_branch_08_conflicting_borrows_across_branches` | `RULE-BRANCH-JOIN-03` | **ACTIVE CONFORMANCE** | Positive: mutually exclusive branch loans accepted; Negative: accessing either referent while post-join carrier live rejected (`E3003`). |
| `sem_borrow_loop_07_borrow_escapes_loop` | `RULE-LOOP-ESCAPE-01` | **ACTIVE CONFORMANCE** | Negative: aggregate-carried iteration local escaping dynamic region rejected (`E3005`). |
| `sem_borrow_loop_08_conditional_borrow_in_loop` | `RULE-LOOP-PATH-02` | **ACTIVE CONFORMANCE** | Positive: conditional iteration-local borrow released at iteration boundary accepted. |
| `sem_borrow_ended_06_mut_stored_shared_after` | `RULE-CARRIED-REF-01` | **ACTIVE CONFORMANCE** | Positive: carried mutable borrow expires after all carriers dead; Negative: transferred carrier retains loan while live (`E3003`). |
| `sem_borrow_ended_08_mut_struct_field_shared_access` | `RULE-CARRIED-REF-02` | **ACTIVE CONFORMANCE** | Positive: disjoint sibling field accessed while struct field holds mut borrow; Negative: conflicting access rejected (`E3003`). |
| `e3005_02_closure_local_borrow` | `RULE-CLOSURE-ESCAPE-01` | **ACTIVE CONFORMANCE** | Negative: closure outliving local referent rejected (`E3005`); Positive: local invocation accepted. |
