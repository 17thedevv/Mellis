# SPEC-HARDENING-02: FFI Reference & Pointer Aggregate Contracts

**Status:** FROZEN & NORMATIVE  
**Authority:** Luna Semantic Core, RFC P6 (Hardened), FFI Lifetime Specification  
**Resolution Scope:** Closes language specification debt for Extern Safe-Reference Returns (`EXTERN-14`) and By-Value Aggregates at FFI Boundary (`EXTERN-03`).

---

## 1. Executive Summary & Problem Formulation

During Phase 2E of the semantic maturity program, 2 conformance fixtures remained under `SPEC-BLOCKED` status:
- `sem_extern_03` (`EXTERN-03`): By-value aggregate crossing the extern boundary.
- `sem_extern_14` (`EXTERN-14`): Extern function returning a safe reference (`&T` / `&rw T`) without an explicit lifetime binding.

While direct raw pointers (`*T`, `*rw T`) and direct safe references (`&T`, `&rw T`) possess frozen parameter semantics (proven in `sem_extern_04`, `sem_extern_05`, `sem_extern_07`, and `sem_extern_08`), aggregates carrying references or raw pointers have lacked a frozen recursive provenance model. Passing an aggregate carrying raw pointers (`struct Holder { p: *rw i32 }`) by value would expose an unproven soundness hole if borrowck does not recursively track foreign provenance escape.

This specification establishes normative baseline contracts for both boundaries under the conservative principle: **aggregates crossing FFI are currently supported only when they contain no provenance-bearing capability transitively**.

---

## 2. RULE-EXTERN-REF-RETURN-01 (`EXTERN-14`): Safe-Reference Returns

### Normative Contract:
An extern declaration must not expose a Luna safe-reference return whose referent lifetime cannot be established by an already-frozen explicit lifetime contract.

Therefore, in the current language baseline:
```rust
extern fn f(...) -> &T;
extern fn f(...) -> &rw T;
```
without such an explicit frozen lifetime contract are statically **REJECTED** by the compiler with:
`error[E2030]: NonFfiSafeType: Extern function signature is not FFI-safe: Extern function returning safe reference is not supported without an explicit lifetime contract`

### Direct Raw-Pointer Return:
Direct raw-pointer returns (`*T` and `*rw T`) carry no safe lifetime capability contract, represent raw unmanaged C-ABI addresses, and remain governed by existing frozen raw-pointer rules (valid and supported).

### Required Controls:
- `extern fn f() -> &T` $\rightarrow$ **Reject** (`E2030`)
- `extern fn f() -> &rw T` $\rightarrow$ **Reject** (`E2030`)
- `extern fn f() -> *T` $\rightarrow$ **Valid**
- `extern fn f() -> *rw T` $\rightarrow$ **Valid**

---

## 3. RULE-EXTERN-AGGREGATE-01 (`EXTERN-03`): By-Value Aggregates at FFI Boundary

### Normative Contract:
An aggregate crossing an extern boundary is valid only when its FFI provenance behavior is already defined by frozen Luna semantics.

Direct raw pointers (`*T`, `*rw T`) and direct safe references (`&T`, `&rw T`) remain supported under their respective direct parameter contracts (synchronous borrows for safe references, tracked escape/`#[sync_noescape]` for raw pointers).

By-value aggregates crossing FFI (whether as parameters or as return values) are currently supported **only when they contain no provenance-bearing reference/pointer capability transitively**.

Aggregates transitively containing:
- `&T`
- `&rw T`
- `*T`
- `*rw T`
- slices or fat pointer capabilities
must be explicitly **REJECTED** by the compiler with:
`error[E2030]: NonFfiSafeType: Extern function signature is not FFI-safe: By-value aggregate containing reference or pointer capability is not supported across FFI boundary`
until recursive aggregate FFI provenance semantics and recursive borrowck escape models are explicitly frozen in a dedicated phase.

### Boundary & Isolation Invariants:
1. **Semantic Restriction, Not ABI-Size Based**: The rejection applies regardless of aggregate memory layout or register passing convention; it is a semantic provenance boundary restriction.
2. **Recursive Provenance Capability Detection vs Callable Signatures**:
   - `contains_ffi_provenance_capability(T)` checks if a **value** of type `T` itself carries reference or raw pointer data provenance. It inspects nominal struct fields, tuple elements, array elements, and enum payloads.
   - It does **not** recurse into callable signatures of function pointers (`fn(...) -> ...`): a function pointer value is a code pointer (instruction address) and does not carry data provenance of arguments it may accept in future invocations.
   - FFI admissibility of callable signatures is enforced orthogonally via `is_ffi_safe_param` and `is_ffi_safe_return`.
3. **No Automatic Raw-Pointer Aggregate Provenance Inference**: Direct raw-pointer parameter contracts do not imply recursive aggregate provenance support. Aggregates containing raw pointers are strictly rejected in the current baseline.
4. **Reference-Free & Pointer-Free Aggregates**: Plain `#[repr(C)]` aggregates containing primitive data (e.g. `Point { x: f32, y: f32 }`) or function pointer callbacks retain their supported behavior (proven in `sem_extern_02` and `sem_extern_03`).

### Required Controls:
- By-value struct parameter containing `&T` $\rightarrow$ **Reject** (`E2030`)
- By-value struct parameter containing `&rw T` $\rightarrow$ **Reject** (`E2030`)
- By-value struct parameter containing `*T` $\rightarrow$ **Reject** (`E2030`)
- By-value struct parameter containing `*rw T` $\rightarrow$ **Reject** (`E2030`)
- By-value nested struct containing raw pointer or safe reference $\rightarrow$ **Reject** (`E2030`)
- By-value tuple / alias wrapping raw pointer or safe reference $\rightarrow$ **Reject** (`E2030`)
- Extern function returning aggregate containing safe reference $\rightarrow$ **Reject** (`E2030`)
- Extern function returning aggregate containing raw pointer $\rightarrow$ **Reject** (`E2030`)
- Struct parameter containing function pointer `fn(i32) -> i32` (`CallbackOnly`) $\rightarrow$ **Valid**
- Struct parameter containing function pointer `fn(*i32) -> i32` (`CallbackPtr`) $\rightarrow$ **Valid** (callable signature, not data provenance)
- Direct `extern fn f(*i32)` and `extern fn f(*rw i32)` $\rightarrow$ **Valid**
- Direct `extern fn f(&i32)` and `extern fn f(&rw i32)` $\rightarrow$ **Valid** (synchronous borrow)
- Reference-free & pointer-free `#[repr(C)]` struct parameter $\rightarrow$ **Valid**

---

## 4. Diagnostic Registry Authority & Provenance History

Both restrictions emit the strongly-typed diagnostic code:
- **Diagnostic Code**: `DiagnosticCode::NonFfiSafeType`
- **Numeric Value**: `2030` (`E2030`)
- **Owning Phase**: `TypeChecker`
- **Span**: Attached to the primary source `Span` of the declaring `extern fn` symbol.

### Registry Provenance History:
- `E2030` (`NonFfiSafeType`) was originally specified and frozen in `docs/diagnostics/diagnostics-v1.md` on **September 12, 2026** (commit `275bedba3856fc6842ae85f3654e7654f629d294`: *"feat(lifetime, semantic): freeze Luna Lifetime Relation Model v1.0 and add Semantic Compliance skill"*).
- The discriminant `NonFfiSafeType = 2030` was committed to `crates/luna-common/src/diagnostic.rs` on **September 18, 2026** (commit `f83486992cad28ac9ca4086a803525c73727e154`: *"fix(diagnostics): close Phase 1C contracts"*).
- SPEC-HARDENING-02 did not invent any new diagnostic numeric code; it activated the pre-existing frozen contract for `E2030`.

---

## 5. Source and `.llib` Parity

The rejection applies symmetrically across compilation phases:
1. **Provider Pre-Emission Rejection**: A provider `.ln` containing unsupported FFI signatures fails compilation during semantic typechecking (`TypeChecker::typecheck_items`), emitting `E2030` and halting before any `.llib` artifact is created.
2. **Consumer Loader Validation**: If an external provider candidate containing an invalid `AstInterface` is evaluated by a consumer, `external.rs` executes `typechecker.typecheck_items(&shifted_provider_items)` during artifact loading. Any non-conforming extern declaration triggers `ExternalComponentError::SemanticFailed` without silent fallback to source.

---

## 6. Conformance Matrix

| Fixture | Frozen Rule ID | Status | Expected Behavior |
| :--- | :---: | :---: | :--- |
| `sem_extern_03_aggregate_carrying_ref` | `RULE-EXTERN-AGGREGATE-01` | **ACTIVE CONFORMANCE** | Negative: aggregates transitively containing `&T`, `&rw T`, `*T`, `*rw T` rejected (`E2030`). Positive: ref/pointer-free `#[repr(C)]` aggregate valid; direct raw-pointer parameters valid. |
| `sem_extern_14_extern_return_reference` | `RULE-EXTERN-REF-RETURN-01` | **ACTIVE CONFORMANCE** | Negative: safe-ref return rejected (`E2030`). Positive: raw-ptr return valid (`*T`, `*rw T`). |
