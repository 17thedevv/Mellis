# Luna v1.0 — Rename Compatibility Contract

**Status**: FROZEN & ADOPTED  
**Date**: September 2026  
**Scope**: Project Identity, Artifact Formats, Resolver Precedence, and ABI Compatibility  

---

## 1. Identity & File Extensions

| Dimension | Canonical (Luna) | Legacy Compatibility (Mellis) | Policy |
|---|---|---|---|
| **Project & Language** | **Luna** | Mellis | Canonical naming everywhere |
| **Compiler CLI** | `luna` (`luna.exe`) | `mellis` | Canonical binary |
| **Source Extension** | **`.ln`** | `.ms` | `.ms` is legacy, `.ln` is canonical |
| **Library Artifact** | **`.llib`** | `.mlib` | `.mlib` is legacy, `.llib` is canonical |
| **Library Crate** | `luna-llib` | `mellis-mlib` | Internal crate naming |
| **Magic Bytes** | `b"LLIB"` | `b"MLIB"` | Writer emits only `LLIB` |

---

## 2. Frozen Runtime ABI Contract (v1.0)

The underlying C runtime symbols and header macros are strictly **FROZEN** as the Runtime ABI v1.0 contract. They are historical ABI identifiers, NOT project branding:

- **Symbol Namespace**: `__mellis_*` (e.g. `__mellis_alloc`, `__mellis_dealloc`, `__mellis_panic`, `__mellis_startup`, `__mellis_start`, `__mellis_shutdown`)
- **Macros & Constants**: `MELLIS_*` (e.g. `MELLIS_ERR_*`, `MELLIS_MAX_ALIGN`)
- **Runtime Library**: `luna-runtime.lib` (with CMake alias `mellis-runtime` and fallback lookup for `mellis-runtime.lib`)

No phase may rename these symbols. They ensure zero breaking changes for existing compiled code and downstream runtime linking.

---

## 3. Artifact Read/Write Contract

### Write Policy (Canonical Only)
- The Luna compiler **ONLY** produces canonical artifacts:
  - Library emitter produces `.llib` files with `LLIB` magic bytes (`LlibHeader`).
  - The compiler **NEVER** writes or generates `.mlib` files or `MLIB` magic bytes.
  - New projects and tests MUST use `.ln` source files.

### Read Policy (Compatibility Fallback)
- The Luna compiler loader and `luna-llib` reader support reading legacy artifacts:
  - Reader accepts both `LLIB` and `MLIB` magic headers.
  - Legacy `.ms` source files and `.mlib` compiled artifacts MAY be read as fallback when no canonical candidates are present.

---

## 4. Resolver Precedence Policy

For any logical provider `foo` (`import "foo";` or `import <foo>;`):

### Resolution Order

```
[Canonical Tier]
  1. foo.llib              (Precompiled canonical library)
  2. foo.ln                (Canonical single source file)
  3. foo/package.ln        (Canonical package directory)
         ↓ (only if ALL canonical candidates are absent)
[Legacy Compatibility Tier]
  4. foo.mlib              (Legacy precompiled library)
  5. foo.ms                (Legacy single source file)
  6. foo/package.ms        (Legacy package directory)
```

### Invariant Rules
1. **Canonical Exists $\implies$ Legacy Ignored**:
   If *any* canonical candidate (`foo.llib`, `foo.ln`, or `foo/package.ln`) exists, it is selected per canonical precedence. Legacy candidates are **completely ignored**.
2. **No Legacy Shadowing**:
   A legacy file CANNOT shadow a canonical file under any circumstance:
   - If both `foo.ln` and `foo.ms` exist $\rightarrow$ **`foo.ln`** is chosen.
   - If both `foo.llib` and `foo.mlib` exist $\rightarrow$ **`foo.llib`** is chosen.
   - If both `foo.ln` and `foo.mlib` exist $\rightarrow$ **`foo.ln`** is chosen (canonical source takes precedence over legacy compiled artifact).
3. **Clean Fallback**:
   Legacy candidates are evaluated *only* when all canonical paths across all search paths return absent.

---

## 5. Environment Variable Precedence

The toolchain inspects environment variables using the following fallback order:

1. **Sysroot**: `LUNA_SYSROOT` $\rightarrow$ fallback `MELLIS_SYSROOT`
2. **Runtime Home**: `LUNA_HOME` $\rightarrow$ fallback `MELLIS_HOME`
3. **Runtime Lib**: `LUNA_RUNTIME_LIB` $\rightarrow$ fallback `MELLIS_RUNTIME_LIB`

---

## 6. Summary for Agents & Developers

- When adding new test files: **Always use `.ln`**.
- When generating library artifacts: **Always use `.llib`**.
- In documentation and diagnostic messages: **Always use Luna, `.ln`, and `.llib`**.
- In runtime symbol definitions: **Never rename `__mellis_*`**.
