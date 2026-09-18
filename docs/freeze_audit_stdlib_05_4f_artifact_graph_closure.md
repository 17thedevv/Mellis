# Freeze Audit: STD-ARCH-01 Phase 4F — Artifact Graph Closure & Source/LLIB Parity

**Status**: Verified & Frozen
**Scope**: `.ln` ↔ canonical `.llib` semantic parity, artifact graph closure, freshness validation
**Depends on**: Phase 4A–4E (frozen)
**Date**: 2026-09-18

---

## 1. Objective

Prove that a provider consumed from source `.ln` or from a canonical fresh `.llib`
has the same semantic identity, public interface, and observable behavior:

```
Luna source provider
    → semantic analysis
    → canonical public/interface metadata
    → .llib serialization
    → dependency metadata
    → freshness validation
    → artifact loading
    → semantic reconstruction
    → consumer compilation
```

Required equivalence (NOT byte/AST/ID equivalence):

- same provider identity, exported symbol identity, types, impls, lifetime contracts, visibility, generic semantics, observable behavior.

---

## 2. Canonical `.llib` representation — semantic authority

A canonical current `.llib` produced by the current writer contains, in addition to
the `Manifest`, both:

- an **`AstInterface`** section (serialized AST arena + items + source), and
- a **`SemanticMetadata`** section (canonical public interface + interface fingerprint).

**`AstInterface` is the required semantic authority for reconstruction.** On load,
the provider AST is relocated and re-analyzed (resolve → typecheck), which re-derives
trait bounds, associated types, lang items, lifetime contracts, and generic parameter
identity from the authoritative AST.

### ARTIFACT-PARITY-01 (canonical scope) — FROZEN

Source and canonical fresh `.llib` are semantically equivalent **for artifacts that
carry the `AstInterface` section**, which the current canonical writer always emits.

### Classification of `SemanticMetadata`-only artifacts

`SemanticMetadata`-only reconstruction (no `AstInterface`) is **semantic-lossy**: the
`CanonicalInterface` does not carry trait bounds (`trait_bounds` / `assoc_type_bounds`),
associated types (`trait_associated_types` / `impl_associated_types`), or lang items.

Therefore:

- The `SemanticMetadata`-only path is **legacy `.mlib` compatibility only** — it exists
  for artifacts produced before `AstInterface` became mandatory.
- A canonical **`.llib` missing the `AstInterface` section is an INVALID artifact** and
  is rejected (`InvalidArtifact`), not silently accepted as a normal representation.
- Legacy **`.mlib`** artifacts without `AstInterface` may still load through the
  `SemanticMetadata` fallback (read compatibility preserved).

This closes the parity gap: the compiler no longer accepts a semantic-lossy
representation as a canonical current artifact.

---

## 3. Associated types — supported scope

- **Within-provider** associated types (trait declaration `type Output;`, impl binding
  `type Output = i32;`, and `T::Output` projection) are supported and survive the
  `.llib` boundary (`test_associated_type_round_trip_canonical_llib`).
- **Cross-provider** associated-type projection (a projection whose impl is owned by a
  different provider) is a **pre-existing general language limitation**: it fails
  identically on the **source** path and on the `.llib` path. Because source and artifact
  behavior are identical, this is not an artifact-parity defect and is out of scope for
  Phase 4F.

---

## 4. Frozen invariants

| Invariant | Statement | Evidence |
| :--- | :--- | :--- |
| **ARTIFACT-IDENTITY-01** | Stable cross-provider symbol identity is `(provider_id, full_logical_path)`; a local name alone is insufficient. | `test_canonical_serialization` (module-scoped + cross-provider same-name) |
| **ARTIFACT-GENERIC-01** | Generic parameter identity is declaration-scoped and survives serialization (`meta::First::T` ≠ `meta::Second::T`). | `test_generic_same_name_params_declaration_scoped`, `test_semantic_metadata_preserves_generic_refs_and_impls` |
| **ARTIFACT-TRAIT-01** | Trait identity, generic bounds/arguments, impl self type, and method metadata survive artifact boundaries. | `test_transitive_trait_method_resolution_a_b_c`, `test_provider_generic_trait_bound_survives_llib`, `test_semantic_metadata_preserves_generic_refs_and_impls` |
| **ARTIFACT-REF-MUTABILITY-01** | `&T` and `&rw T` remain semantically distinct through artifacts. | `test_semantic_metadata_preserves_generic_refs_and_impls`, stdlib IntoIterator modality suites |
| **ARTIFACT-LIFETIME-01** | Canonical `life_from(...)` / `outlives(...)` contracts survive artifact boundaries without relying on session-local IDs. | `lifetime_relation_abi_acceptance_tests`, `test_transitive_lifetime_contract_a_b_c` |
| **ARTIFACT-DEPENDENCY-01** | Dependency metadata uses canonical logical provider identity, not physical paths. | `DependencyEntry{provider_name, interface_fingerprint}`; 25-provider graph |
| **ARTIFACT-DEPENDENCY-VALIDATION-01** | Freshness compares the dependency's actual resolved canonical interface fingerprint; a placeholder/default fingerprint is never valid evidence. | `test_freshness::test_dependency_interface_change_invalidates_consumer` |
| **ARTIFACT-VALIDATION-ORDER-01** | Dependency-sensitive validity is (re)validated after required dependency interfaces are loaded. | `external.rs` post-import revalidation; `test_freshness` |
| **ARTIFACT-FRESHNESS-01** | Existing invalid/stale artifact is rejected; it MUST NOT silently fall back to source. | `test_freshness`, `test_strict_artifact_rejection`, `test_canonical_llib_missing_ast_interface_rejected` |
| **ARTIFACT-PARITY-01** | Fresh source and canonical `.llib` are semantically equivalent (canonical = carries `AstInterface`). | `test_mixed_source_artifact_graph_parity`, `test_artifact_metadata_parity`, stdlib `*_parity` |
| **ARTIFACT-ORCHESTRATION-01** | The compiler validates; application/build tooling (`SysrootBuilder`) orchestrates rebuilds. Normal compilation never auto-rebuilds. | architecture boundary; no rebuild logic in loader |

---

## 5. Artifact graph closure (25 / 25)

Canonical sysroot rebuilt through `SysrootBuilder` (no manual artifact creation/copy):

- canonical providers: **25**
- `.ln`: **25**
- `.llib`: **25**
- `.obj`: **25**
- DAG: 25 nodes / 61 edges; no self-edges, no unknown deps
- headers valid; dependency identities resolve; interface fingerprints validate
- no orphan `.obj`; no legacy monolithic artifacts (`core/alloc/io.llib/.obj`)
- no `.build.lock`; no temp publication files

### `core/panic`

`core/panic` is the **canonical logical provider ID** (established in Phase 4A). It is
deliberately spelled with the `core/` prefix and there is **no `panic` alias**. This is
intentional frozen architecture, not an anomaly — do not rename or alias it.

---

## 6. Test-infrastructure defect fixed

Six stdlib/vec acceptance suites published freshly compiled providers into the shared
canonical sysroot with a **non-atomic `fs::copy`**, racing with concurrent readers under
`cargo test --workspace` (parallel). This produced intermittent failures in unrelated
binaries. Fix: publish via **temp + `fs::rename`** (atomic) at all six sites.

Note: `interface_fingerprint` was verified deterministic (`fp0 == fp1 == canonical`), so
only the non-atomic read window caused the flake — not fingerprint instability.

---

## 7. Verification

- Targeted: `test_artifact_metadata_parity` (9), `test_freshness` (3),
  `test_strict_artifact_rejection` (5), `test_canonical_serialization` (2),
  `test_sysroot_build_invariants` (5), `test_core_bootstrap_isolation` (10), plus
  representative stdlib parity (Option/Result, Slice, Vec, String, HashMap, HashSet, IO,
  internal-provider transitive loading).
- Workspace serial: **126 test binaries ok, 0 failed**.
- Workspace parallel: **126 test binaries ok, 0 failed**.

---

## 8. Boundary compliance

No new language semantics · no provider-specific artifact hack · physical path never a
semantic identity · no source fallback for invalid artifacts · no compiler auto-rebuild ·
no visibility/provenance change · no new lang contract · no runtime ABI change · Phase 4D
publication unchanged · Phase 4E unchanged · Phase 4G not started.
