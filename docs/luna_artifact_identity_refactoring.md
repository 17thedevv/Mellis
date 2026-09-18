# Luna Artifact Identity Refactoring

Refactor Luna's external artifact resolution pipeline around the architectural principle: **"Luna trusts identity, never existence."**

The architectural boundary is strictly defined as: **"The compiler validates; the application orchestrates."**
The Luna compiler is unaware of package-management policy. Artifact validation is a compiler capability; rebuilding, caching, acquisition, versioning, and invalidation policy belong to application-layer tooling.

This will be accomplished in incremental phases, ensuring tests are added and behavior remains correct at every step.

> **Canonical vs legacy (Phase 4G).** The canonical current artifact is `.llib`
> (magic `LLIB`) and it must carry an `AstInterface` section — the required semantic
> authority. `.mlib` (magic `MLIB`) is a **legacy read-compatible** format only, and
> the `SemanticMetadata`-only reconstruction path exists for legacy `.mlib`
> artifacts. Canonical tooling never writes `.mlib`. See
> [std-arch-01-compatibility-matrix.md](./std-arch-01-compatibility-matrix.md).

## Proposed Changes

We will execute this refactoring in the following phases.

### Phase 0: Baseline (Completed)

Before changing any compiler behavior, we wrote baseline regression tests in `crates/luna-driver/tests/test_artifact_resolution_baseline.rs` to document and freeze the current resolution behavior.

- Created a test suite testing:
  - Loading `.llib` only
  - Loading `.ln` only
  - Behavior when both `.llib` and `.ln` are present (currently `.llib` wins blindly)
  - Behavior with dependency changes
  - Behavior across context/compiler changes (currently blindly ignored)
- The compiler semantics were **not** changed in this phase.

### Phase 1: Artifact Identity Model

- Introduce strongly-typed identity models in `luna-llib` and `luna-driver`.
- Differentiate between:
  - `SourceFingerprint`
  - `InterfaceFingerprint`
  - `DependencyInterfaceFingerprint`
  - `CompilerIdentity`
  - `LanguageIdentity`
  - `TargetIdentity`
  - `FeatureConfigurationIdentity`
  - `ArtifactKind`
- Migrate from opaque `[u8; 32]` or arbitrary strings to typed wrappers. Ensure the internal representation is clear before serialization.

### Phase 2: Canonical Public Interface Fingerprint

- Audit the existing `interface_hash`.
- Implement a deterministic serialization mechanism that only covers the *canonical public interface* (exported symbols, public signatures, nominal types, traits, impls).
- Verify with tests that private implementation changes preserve the fingerprint, while public signature changes modify it.

### Phase 3: Dependency Interface Identities

- Extend the `.llib` manifest so that it explicitly stores a `DependencyInterfaceIdentity` for every direct semantic dependency.
- Structure: `{ module_provider_identity, interface_fingerprint }`. Do not use a generic string-to-hash map.
- This ensures precise downstream invalidation.

### Phase 4: Compilation Context Identity

- Formalize the identities within `luna-driver`. Do not flatten everything into a single hash immediately.
- Separate components:
  - `CompilerIdentity` (compiler implementation/version)
  - `LanguageIdentity` (Luna language/spec version)
  - `CompilationContext` (Aggregates compiler + language + target + features + relevant config)
- This allows granular reporting (e.g. "Target changed" vs "Compiler version changed").

### Phase 5: Generic Artifact Validation

- Introduce artifact-validation primitives in **`luna-llib`** (since it owns the artifact domain).
- The validator returns `Valid` or structured invalid reasons (e.g., `InterfaceMismatch`, `TargetMismatch`).
- **Validation only returns status; it does not trigger rebuilds.**

### Phase 6: Driver Integration

- Modify `ExternalComponentLoader` and `DriverSession` in `luna-driver`.
- **Discovery**: Only locates candidates (`.llib` or `.ln`).
- **Validation**: Validator checks `.llib` against the expected context.
- **Strict Rejection Semantic Rule**:
  - If `.llib` is **present but invalid**: `REJECT` (do NOT use `.llib`, do NOT fallback to `.ln`, do NOT rebuild).
  - If `.llib` is **absent** and `.ln` is **present**: proceed with normal source compilation via discovery semantics.
- This creates the hard boundary: the compiler either successfully loads a valid artifact or fails loudly, handing responsibility back to the package manager (`mpm`).

### Phase 7: Application-layer integration (mpm/build orchestration, future)

- The application layer (e.g., `mpm` or future `luna-cli` commands) will consume the compiler's validation API.
- Upon receiving an `InvalidReason`, the application layer decides the rebuild policy (delete, re-invoke compiler to rebuild, fetch from cache).

### Phase 8: Object Code Identity

- Ensure that native object-code validity is evaluated separately from semantic validity.
- Model this distinction clearly to account for codegen flags and targets.

## Verification Plan

We will run the existing regression test suite after every phase. Additionally, we will build a dedicated test harness testing the regression cases outlined in the specification.
