---
name: luna-stdlib-migration
description: Governs migration from monolithic stdlib providers to component-level providers without changing semantics.
---

# Luna Stdlib Architecture Migration

## Purpose

This skill governs migration from monolithic stdlib providers to component-level providers.

This is a structural migration.

Preserve behavior first. Improve architecture second.

---

## 1. Migration Rule

Every extracted component must preserve its previous semantics.

Example:

```text
alloc.ln
```

may be decomposed into:

```text
alloc/box.ln
alloc/vec.ln
alloc/string.ln
alloc/raw_table.ln
alloc/hashmap.ln
alloc/hashset.ln
```

The migration MUST NOT change their APIs unless explicitly required by another frozen feature.

## 2. Migration Order

Recommended order:

Step 1 — Inventory

Identify:

declarations
impl blocks
lang items
internal helpers
module declarations
imports
dependencies
tests
artifact generation paths

Do not move files before understanding dependency boundaries.

Step 2 — Define Component Ownership

Assign each declaration to exactly one component.

Example:

Vec           → alloc/vec.ln
String        → alloc/string.ln
Box           → alloc/box.ln
RawTable      → alloc/raw_table.ln
HashMap       → alloc/hashmap.ln
HashSet       → alloc/hashset.ln

Step 3 — Introduce Explicit module std

Declarations intended for public standard-library namespace should live under:

module std {
    ...
}

Step 4 — Split Providers

Move declarations without semantic changes.

Step 5 — Build Each Provider

Generate canonical .llib artifacts.

Step 6 — Remove Monolithic Providers

Only remove old providers after all canonical consumers resolve successfully.

Step 7 — Validate

Run the architecture and regression matrix.

## 3. Dependency Direction

The decomposition must preserve dependency direction.

For example:

RawTable
   ↑
HashMap
   ↑
HashSet

must remain structurally representable.

Do not solve dependency cycles by adding compiler special cases.

Use explicit imports/provider composition.

## 4. Internal Helpers

When extracting a component, helper functions used only by that component should remain local unless another component explicitly requires them.

Do not accidentally enlarge the public API during migration.

## 5. Public API Preservation

Before and after migration, compare:

exported symbols
module paths
trait implementations
method signatures
lifetime contracts
ownership behavior
diagnostics where frozen

The refactor must not accidentally turn private implementation details into public API.

## 6. .ln / .llib Migration

For every migrated component:

source provider
    ↓
official compiler/build path
    ↓
canonical .llib

Never manually construct artifacts.

Never assume a .llib is current merely because it exists.

A stale artifact must not silently mask a source migration problem.

## 7. Canonical Provider Resolution

When both representations exist:

foo.ln
foo.llib

they represent one logical provider.

Normal compiled resolution should use the valid/fresh .llib.

The compiler MUST NOT silently load both representations.

If the .llib is stale or invalid:

reject

or invoke the explicit build/rebuild path.

Do not silently fall back to source and pretend the artifact is valid.

## 8. Testing Boundary

Stdlib behavior must be tested at the Luna CLI boundary.

Prefer:

tests/luna/stdlib/...

over compiler-internal tests for ordinary stdlib behavior.

Compiler-internal Rust tests are appropriate only when testing a compiler subsystem itself.

For this migration, test:

source provider path
.llib provider path
multi-provider std merge
no provider-derived namespace
language-contract auto-loading
regression behavior of existing stdlib APIs

## 9. Artifact Hygiene

Before declaring success:

rebuild migrated .llib artifacts
remove obsolete canonical artifacts
verify resolver precedence
verify no stale artifact is selected
verify source/.llib parity

Do not claim migration complete merely because source tests pass.

## 10. Forbidden Shortcuts

Do NOT:

add compiler special cases for Vec/String/HashMap/etc.
hardcode provider filenames into semantic type logic
make provider names into namespaces
auto-import all core/alloc declarations
add const generics as part of this refactor
change lifetime syntax
change borrow semantics
redesign trait coherence
introduce new runtime ABI symbols
rename frozen __mellis_* runtime ABI symbols
preserve old and new compiler implementations in parallel

There must be one canonical Luna implementation.

## 11. Completion Criteria

Migration is complete only when:

canonical component files exist
canonical artifacts exist
monolithic providers are no longer canonical
explicit std module composition works
language contracts auto-load correctly
ordinary core/alloc/io APIs remain explicit
source/.llib parity passes
existing stdlib regression suites pass
no compiler container-specific branches were introduced
no frozen semantic contract was changed
