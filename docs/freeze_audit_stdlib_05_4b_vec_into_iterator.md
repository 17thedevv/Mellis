# Freeze Audit: Stdlib-05.4B Vec IntoIterator

**Status**: Verified & Frozen
**Component**: `alloc` (Vec IntoIterator)
**Date**: 2026-09-16

## 1. Objective
Ensure proper ownership transfer and iteration parity for `Vec<T>`, `&Vec<T>`, and `&rw Vec<T>`.

## 2. Issues Encountered & Resolved
During testing, a compiler bug (C-GAP-COHERENCE) was discovered where modality information (mutability and reference vs owned) was lost during trait overlap detection and `.llib` serialization.
- **Root Cause 1**: `coherence.rs` used a flawed tuple-pattern matcher that discarded `Mutability`. This was resolved in Part 1.
- **Root Cause 2**: `metadata_builder.rs` grouped `ImplHeader` entries by `ExternalImplKey` (which intentionally strips modality), causing multiple trait implementations for different modalities of the same nominal type to overwrite each other. Furthermore, `metadata_decoder.rs` did not persist `method_self_types`, causing the method resolution to fallback incorrectly.
- **Resolution**:
  - `ProviderInterface` was updated to explicitly serialize/deserialize `method_self_types`.
  - `metadata_builder.rs` was refactored to emit exactly one `ImplHeader` per exact `self_type`.
  - `metadata_decoder.rs` was updated to synthesize unique symbol paths using the `self_type` index (e.g. `format!("{}#{}", m_name, impl_header.self_type)`) to prevent local symbol collisions. **Note**: This `symbol_path` is exclusively for metadata-local decoder disambiguation, it is *not* a canonical semantic mangling (which is handled by `CanonicalInstanceIdentity`).

## 3. Verification (Rule 12 Compliance)
All stdlib acceptance tests (`test_vit1` through `test_vit12`) pass using the `luna` CLI harness (via Rust integration test proxy).
Crucially, a comprehensive E2E `.ln` test (`scratch/test_05_4b.ln`) explicitly validating `Vec<T>`, `&Vec<T>`, and `&rw Vec<T>` `IntoIterator` logic succeeded against the newly compiled `alloc.llib` and `core.llib`, proving that multiple modality impls for the same nominal base type are correctly preserved and resolved across the `.llib` serialization boundary.

## 4. Architectural Invariants Frozen
- **META-MODALITY-1**: Provider metadata MUST preserve the complete semantic `self_type` of every impl method receiver. Two impls that differ only by reference mutability MUST remain distinguishable across source → `.llib` serialization and `.llib` → semantic-table deserialization.
- **META-MODALITY-2**: Serialization/deserialization MUST NOT collapse multiple impls sharing the same nominal base type but differing in semantic reference modality.

## 5. Artifact Compatibility
- Binary format/parser compatibility is preserved, but old artifacts may have lossy modality metadata and therefore do not provide the corrected semantics until rebuilt.
- Format compatibility: YES
- Semantic modality compatibility of legacy artifacts: NO
- Rebuild required to obtain corrected modality metadata: YES

## 6. Stdlib-Compiler Boundary
No standard library specific workarounds were created. The compiler fix generically supports any user-defined nominal type with multiple modality-specific trait implementations.
