---
name: luna-testing-strategy
description: Canonical testing strategy for the Luna language, compiler, and standard library. Enforces testing at the highest public abstraction boundary, preferring Luna CLI + .ln fixtures for language/stdlib behavior, while reserving cargo test for compiler-internal invariants.
---

# Luna Testing Strategy

## Purpose

This skill defines the canonical testing strategy for the Luna language, compiler, and standard library.

The primary rule is:

> **Test a behavior at the highest public abstraction level that can faithfully verify it.**
>
> Do not use lower-level Rust tests merely because they are easier to implement.

---

## 1. Testing Layers

Luna testing is divided into four distinct layers.

### Layer A — Compiler Unit / Integration Tests

Use `cargo test` for compiler-internal invariants and implementation-specific behavior.

**Subsystems:**
- Resolver
- Parser internals
- Typechecker
- Trait coherence
- Borrow checker
- Lifetime analysis
- Monomorphization
- MVIR generation
- Optimizer
- LLVM backend
- Diagnostics
- Metadata encoding/decoding
- Compiler infrastructure

**Typical Location:**
```
crates/*/tests/
crates/*/src/*_tests
```

**Usage:** Use this layer when the behavior itself is a compiler subsystem invariant.

**Examples:**
- `Reference(&T)` and `Reference(&rw T)` must remain distinct in trait coherence.
- `CanonicalInstanceIdentity` must produce distinct mangled symbols.
- Borrowck must reject a specific invalid borrow state.

These are legitimate compiler-level tests.

---

### Layer B — Luna Language / Stdlib Acceptance Tests

For language-visible and standard-library behavior, prefer standalone Luna source fixtures executed through the `luna` CLI.

**Rule:** Do NOT create `crates/luna-driver/tests/stdlib_*.rs` merely because Rust test helpers are convenient.

**Preferred Structure:**
```
tests/luna/
├── stdlib/
│   ├── collections/
│   ├── iterators/
│   ├── ownership/
│   ├── strings/
│   └── io/
└── language/
```

**Examples:**
- `tests/luna/stdlib/iterators/map_basic.ln`
- `tests/luna/stdlib/iterators/filter_drop.ln`
- `tests/luna/stdlib/iterators/collect_vec.ln`
- `tests/luna/stdlib/iterators/for_in.ln`

Tests must use the same public surface available to an end-user Luna program.

**Typical Verification Flow:**
```
.ln source
    ↓
luna CLI
    ↓
compile
    ↓
link
    ↓
execute
    ↓
stdout / stderr / exit code
```

**Prefer this layer for:**
- Standard library APIs
- Iterator behavior
- Collection behavior
- Ownership behavior visible to users
- Function-level language semantics
- Generic API usage
- Public diagnostics
- Source programs using the standard library

---

### Layer C — Source / .llib Parity

When a feature is available both from Luna source and from a precompiled `.llib`, verify both paths.

**Canonical Pattern:**
- **Fixture A:** resolve stdlib from source `.ln`
- **Fixture B:** resolve stdlib from `.llib`
- **Expected:** same semantic result, same stdout, same exit code

Do not test parity by bypassing the real public loading path.

**Purpose:** Prove that `.ln` implementation and `.llib` artifact expose the exact same public behavior.

---

### Layer D — Regression / Freeze Harness

Regression harnesses may orchestrate large numbers of CLI fixtures.

The harness is an orchestration layer, not the semantic test itself.

- **Correct:** Rust harness → invoke `luna` → execute `.ln` fixture → inspect exit code/output.
- **Incorrect:** Rust harness → instantiate Driver internals → construct compiler context manually → inspect internal semantic state (unless the test is explicitly a compiler-internal test).

---

## 2. Decision Rule

Before creating a test, ask:

1. **Question 1:** Can this behavior be expressed and verified through a standalone `.ln` program using `luna`?
   - If **YES** → Use a Luna CLI fixture.
2. **Question 2:** Does the test need to inspect an internal compiler representation or invariant that is not visible through the language?
   - If **YES** → Use a Rust compiler test (`cargo test`).
3. **Question 3:** Is the behavior specifically about `.llib` generation/loading?
   - If **YES** → Use a CLI E2E test exercising the real `.llib` path, optionally orchestrated by Rust.
4. **Question 4:** Is the test proving a compiler gap?
   - If **YES** → Create a dedicated compiler-gap test suite using user-defined Luna types and compiler-level assertions where necessary. The compiler-gap test must remain independent of the stdlib feature that originally exposed the gap.

---

## 3. Stdlib Testing Rules

For stdlib phases:
- Prefer `.ln` fixtures.
- Execute through the `luna` CLI.
- Test only public APIs.
- Do not use internal driver helpers to simulate user behavior.
- Do not create stdlib-specific compiler branches to satisfy tests.
- Test source and `.llib` paths separately when relevant.
- Include negative cases where ownership, lifetime, visibility, or type rules matter.
- Test observable behavior, not implementation details.

---

## 4. Ownership / Lifetime Testing

For ownership-sensitive stdlib features, successful execution is not sufficient evidence.

1. **Owned Resources:** Tests should establish `created == dropped` where applicable.
2. **Borrowed APIs:**
   ```
   borrow alive → conflicting mutation rejected
   borrow ends  → mutation accepted
   ```
3. **Borrowed Iteration:**
   ```
   collection → iterator → &T / &rw T
   ```
   Must preserve provenance and must not transfer ownership of `T`.
4. **Owned Iteration:**
   ```
   collection → owning iterator → T
   ```
   Must transfer ownership exactly once.
5. **Integrity Rule:** Never modify expected results to hide a lifetime, ownership, or drop bug.

---

## 5. Compiler Gap Protocol

If a stdlib feature fails because of an apparent compiler limitation (e.g., `E_UNRESOLVED_SYMBOL` or `E_TRAIT_BOUND_NOT_SATISFIED` when the declaration is known to exist):
1. **Preflight: Artifact Freshness Check.** A source stdlib change is not considered available to CLI tests until its canonical `.llib` artifact has been regenerated through the official build path (e.g. `luna build libs/external/alloc.ln --emit llib --lib`). Verify that the `.llib` is fresh before assuming a compiler issue.
2. **STOP** stdlib implementation.
3. Do not add a compiler special case.
4. Determine whether the missing behavior is a generic language capability.
5. Reproduce the issue using a user-defined type independent of stdlib.
6. Create an isolated `C-GAP-*` test.
7. Fix and freeze the generic compiler capability.
8. Resume the stdlib phase afterward.

```
Stdlib feature
    ↓
compiler limitation suspected
    ↓
PREFLIGHT: Check Artifact Freshness (.llib)
    ↓
STOP
    ↓
user-defined reproducer
    ↓
C-GAP
    ↓
generic compiler fix
    ↓
compiler regression
    ↓
freeze
    ↓
resume stdlib
```

---

## 6. Test Naming

Use names that describe semantic intent.

- **Good:**
  - `iter_collect_owned_items_drop_exactly_once`
  - `for_in_borrowed_collection_locks_mutation`
  - `coherence_distinguishes_shared_and_mutable_reference_impls`
- **Avoid:**
  - `test_helper_foo_works`
  - `test_internal_driver_path`

---

## 7. Freeze Requirements

A feature can be marked **FROZEN** only after:
1. Public behavior is tested.
2. Relevant negative behavior is tested.
3. Ownership/lifetime invariants are tested when applicable.
4. Source / `.llib` parity is tested when applicable.
5. Compiler-specific changes are isolated and justified.
6. No stdlib-specific compiler branch exists.
7. Full relevant regression suites pass.
8. The freeze document records the semantic invariants and evidence.

*Passing tests alone does not establish a semantic freeze.*

---

## 8. Anti-Patterns

**Never:**
- Create Rust stdlib tests simply because they are easier.
- Test stdlib internals through private compiler APIs.
- Modify expected outputs to hide semantic bugs.
- Add compiler magic for a single stdlib type.
- Treat compilation success as proof of ownership correctness.
- Treat runtime success as proof of lifetime correctness.
- Claim a feature is generic without testing a user-defined case when the claim concerns compiler capability.

---

## 9. Canonical Preference Hierarchy

When multiple testing approaches can verify the same behavior, prefer:
1. **Luna CLI + `.ln` fixture**
2. **CLI + `.llib` E2E**
3. **Rust integration harness invoking the CLI**
4. **Rust compiler-level test**

Move lower in the hierarchy only when the higher layer cannot faithfully observe the invariant.

The goal is not maximum test count.

> **The goal is to verify each semantic claim at the correct architectural boundary.**
