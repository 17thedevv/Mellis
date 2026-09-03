# Phase 16 v2 — Comptime + Reflection

**Status: COMPLETE — Phase 16.5 PASSED**

- Design: FROZEN
- Implementation: DONE
- Freeze Gate Audit: ✅ PASSED
- Phase 16.5: ✅ COMPLETE — IntrinsicId dispatch fixed
- See: [docs/Phase16.5_IntrinsicDispatch_Plan.md](docs/Phase16.5_IntrinsicDispatch_Plan.md)

Compile-time execution, validation, and reflection for the Mellis language.

---

## Overview

`comptime` is not "full runtime at compile time."

It is:

> Compile-time evaluation of the supported Mellis semantic subset with owned evaluator state, reflection capabilities, and strict phase-boundary restrictions.

---

## Implementation Status

### ✅ Completed

| Component | Status | Notes |
|-----------|--------|-------|
| AST Interpreter Removal | ✅ Done | `ComptimeEvaluator` in `mellis-semantic/src/comptime/interp.rs` removed entirely |
| Typechecker Offload | ✅ Done | Typechecker no longer falls back to `ComptimeEvaluator` for array sizes/enum discriminants; lowers to MVIR |
| Intrinsic Interception | ✅ Done | Typechecker identifies intrinsic functions via `#[intrinsic]` attribute; populates `expr_intrinsics` map |
| MVIR Interpreter: Purity Checking | ✅ Done | `Instruction::CallDirect` verifies target is side-effect-free; `ComptimeError::ForbiddenSideEffect` on violation |
| MVIR Interpreter: Type Representation | ✅ Done | `RuntimeValue::Type` added; conversion to/from `ComptimeValue::Type(TypeRepr)` established |

### ⏳ Deferred to Phase 17

| Capability | Reason |
|------------|--------|
| `type_info` MVIR instruction generation | Requires `TypeInfoStruct` layout in `mellis-mlib` (stdlib) |
| `type_of` MVIR instruction generation | Requires `TypeInfoStruct` layout in `mellis-mlib` (stdlib) |

Per the design contract: "Runtime RTTI embedding ... are deferred capabilities." The infrastructure is in place; the stdlib integration comes in Phase 17.

### Validation

```
cargo check --workspace          ✅ PASS
cargo test -p mellis-mvir        ✅ PASS (no regressions)
cargo test -p mellis-semantic    ✅ PASS (no regressions)
cargo test -p mellis-borrowck    ✅ PASS (no regressions)
cargo test -p mellis-optimizer   ✅ PASS (no regressions)
cargo test -p mellis-driver      ✅ PASS (no regressions)
```

All Phase 16.5 validation passed 100%.

### Known Deviation

None. Phase 16.5 resolved the IntrinsicId dispatch deviation.

---

## A. Purpose

Comptime enables:

- Compile-time computation of constant expressions
- Type-level validation and generic instantiation
- Reflection metadata extraction (`type_info`, `size_of`, etc.)
- Compile-time macro expansion (code → code, via `macro` keyword)

Comptime does NOT enable:

- IO, networking, or runtime state mutation
- Async execution
- Runtime reflection embedding
- Any operation that violates phase boundaries

---

## B. const vs comptime

| Keyword | Meaning |
|---------|---------|
| `const` | Compile-time value (statically known) |
| `comptime` | Explicit compile-time execution context |

Example:

```mellis
const MAX_SIZE: u32 = 1024;           // compile-time constant
comptime fn validate_size(n: u32) {   // explicit comptime function
    if n > MAX_SIZE {
        comptime_error("size exceeds maximum");
    }
}
```

A `const` value can be used anywhere a compile-time value is expected. A `comptime` block explicitly requests evaluation during compilation.

---

## C. Execution Model

```
Valid Mellis Source
        ↓
  Type Checking
        ↓
       MVIR
        ↓
Comptime Interpreter
```

Comptime executes MVIR (Mellis Virtual IR), not AST. The evaluator is a pure interpreter over validated MVIR.

**Pipeline:**
1. Parse → AST
2. Resolve types → validated AST with semantic types
3. Generate MVIR
4. Comptime interpreter executes MVIR
5. Reflection queries resolve from canonical type metadata

---

## D. Type Model

```
TypeValue = opaque SemanticTypeId
```

A `TypeValue` represents a compile-time type. It is an opaque identifier that the evaluator tracks but cannot directly inspect as a runtime value.

- `SemanticTypeId` is owned by the compiler
- The evaluator holds `TypeValue` (an alias to `SemanticTypeId`)
- Reflection intrinsics resolve metadata from the `SemanticTypeId`

---

## E. SemanticTypeId Canonicalization

The following are NOT implementation details — they are semantic guarantees:

```
B1. Every semantically valid type has canonical identity.
B2. Generic instantiations are canonicalized to distinct semantic identities.
B3. Reflection resolves metadata from canonical semantic type identity.
```

`SemanticTypeId` may be canonicalized through interning/type registry at any phase. The contract does not mandate "registration during AST resolution."

Implementation may:
- Use a global type registry
- Use structural hashing
- Use intern pools

The only requirement is that equivalent types produce the same `SemanticTypeId`.

---

## F. Reflection

### Intrinsics

```
type_of(value: T) -> TypeValue
type_info(type: TypeValue) -> TypeInfo
size_of(type: TypeValue) -> u64
align_of(type: TypeValue) -> u64
```

Intrinsics are dispatched by compiler-owned `IntrinsicId`. The evaluator does NOT know stdlib names.

```
ComptimeContext {
    // Representation is implementation detail.
    // Contract: evaluator dispatches intrinsics by IntrinsicId.
}
```

### Reflection Metadata

```mellis
struct TypeInfo {
    name: []const u8,
    kind: TypeKind,
    layout: LayoutInfo,
    fields: []const FieldInfo,
    variants: []const VariantInfo,
}

struct FieldInfo {
    name: []const u8,
    type: TypeValue,
    offset: u64,
}

struct VariantInfo {
    name: []const u8,
    tag: u64,
    fields: []const FieldInfo,
}

struct LayoutInfo {
    size: u64,
    alignment: u64,
    is_packed: bool,
    is_scalar: bool,
}

enum TypeKind {
    Struct,
    Enum,
    Array,
    Tuple,
    Primitive,
    Generic,
    Function,
}
```

---

## G. Capability Boundary

For every MVIR instruction, classify:

| Classification | Meaning |
|----------------|---------|
| **COMPTIME_ALLOWED** | Instruction fully supported in comptime context |
| **COMPTIME_RESTRICTED** | Allowed with specific restrictions (see below) |
| **COMPTIME_FORBIDDEN** | Instruction results in `ComptimeError` |

### COMPTIME_ALLOWED

- Arithmetic on primitive types (iadd, isub, imul, fadd, fsub, etc.)
- Comparison operations
- Struct/array/tuple construction
- Field projection
- Variant construction and projection
- Local variable binding and lookup
- Basic block branching (br, br_if)
- Return
- Call (intrinsics, comptime functions)
- Casts between primitive types
- Aggregate operations (extract, insert)

### COMPTIME_RESTRICTED

| Operation | Restriction |
|-----------|-------------|
| **Load from pointer** | Only from valid in-memory locals or temporaries |
| **Store to pointer** | Only to local temporaries; no external mutation |
| **Reference creation** | Reference cannot escape comptime scope |
| **Borrow** | Internal borrow chains allowed; must be resolved before exit |
| **Generic instantiation** | Must produce canonical SemanticTypeId |

### COMPTIME_FORBIDDEN

- IO operations (file, network, stdin/stdout)
- Async operations
- Thread spawning
- Runtime state mutation
- Any effect not explicitly listed as allowed

### Error Handling

```
Unsupported comptime instruction:
    → ComptimeError
    → never a runtime fallback
```

No MVIR instruction may silently fall through to a default evaluator behavior. Every instruction must have a defined comptime semantics.

---

## H. Layout

One canonical `LayoutModel` is shared between:

- Comptime interpreter (for reflection)
- MVIR type system
- LLVM backend

Layout is computed once and cached by `SemanticTypeId`.

---

## I. Generic Types

```
Generic instantiation → canonical SemanticTypeId
```

When a generic type is instantiated with concrete type arguments, the result is a distinct `SemanticTypeId` that is canonicalized in the type registry.

Example:

```mellis
struct Box<T> { value: T }

comptime fn test() {
    // Two instantiations with same args → same SemanticTypeId
    let a: Box<u32>;
    let b: Box<u32>;
    // a and b have identical type identity
}
```

---

## J. Owned Values

Comptime operates on **owned values**, not references:

- Struct/enum/array/tuple values are copied by value
- Generic values carry their concrete type identity
- No shared mutable state

The evaluator maintains **owned evaluator state** — no external side effects.

---

## K. Borrowing

```
Temporary/internal borrow: allowed within comptime scope
Reference escaping comptime: FORBIDDEN
```

Example:

```mellis
comptime fn test() {
    let s = Struct { ... };
    let r = &s;           // internal borrow: OK
    // r cannot be returned or stored outside comptime
}
```

The evaluator tracks borrow lifetimes and validates that no reference escapes.

---

## L. Effects

| Effect | Comptime Allowed |
|--------|------------------|
| IO (file, network) | ❌ FORBIDDEN |
| Runtime state mutation | ❌ FORBIDDEN |
| Async | ❌ FORBIDDEN |
| Arithmetic | ✅ ALLOWED |
| Type reflection | ✅ ALLOWED |
| Compile-time computation | ✅ ALLOWED |
| Diagnostic emission | ✅ ALLOWED |

Comptime is pure computation over the program's static structure.

---

## M. Resource Limits

To prevent infinite compilation loops:

- **Steps limit**: Maximum MVIR instructions per comptime evaluation
- **Memory limit**: Maximum value graph size
- **Depth limit**: Maximum call stack depth
- **Iteration limit**: Maximum loop iterations

All limits are configurable. Exceeding any limit produces a `ComptimeError`.

---

## N. Macro Boundary

| Concept | Definition |
|---------|------------|
| `macro` | Code → code transformation |
| `comptime` | Code → value/validation/metadata |

A macro operates on the AST, producing new AST nodes. A comptime block operates on values, producing compile-time results.

These are distinct phases with distinct semantics.

---

## O. TypeValue Contract

```
TypeValue is compile-time-only.
TypeValue cannot be embedded at runtime.
```

A `TypeValue` exists only during compilation. It cannot be:
- Stored in a runtime struct field
- Passed across the comptime/runtime boundary
- Printed or serialized at runtime

---

## P. Implementation Approach

### Semantic Rewrite, Code Reuse Opportunistically

```
Semantic model: NEW / FROZEN
Implementation: reuse only if compatible
```

### Audit Before Reuse

Before reusing any component from Phase 16 v1:

1. **Audit**: Does this component satisfy v2 invariants?
2. **Prove compatibility**: Document why the component is sound under the new model
3. **If incompatible**: Replace, do not patch

### Component Classification

| Component | Action |
|-----------|--------|
| Old diagnostics | ✅ Reuse if sound |
| Old resource limits | ✅ Reuse if sound |
| Old call stack | ✅ Reuse if sound |
| Old AST interpreter | ❌ Delete/Replace |
| Old builtin-name dispatch | ❌ Delete/Replace |
| Old primitive-only value model | ❌ Replace |
| Old ownership-blind context | ❌ Replace |

### Not Allowed

- "Rewrite everything from scratch" (may waste valid work)
- "Patch the existing evaluator" (semantic model has changed)

### Required

- Prove each reused component is compatible with v2 semantics
- Document the compatibility justification

---

## Q. Freeze Gate

After implementation, a separate audit must verify:

```
"Mellis → MVIR → Comptime" pipeline is complete.
```

Questions to answer:
- Does every comptime operation go through MVIR?
- Is the AST interpreter eliminated?
- Are all hardcoded paths removed?
- Do all intrinsics use `IntrinsicId` dispatch?

The freeze gate is passed only when the pipeline is verified.

---

## R. Pipeline

```
DESIGN FROZEN
      ↓
Implementation Contract
      ↓
Code Audit of Phase 16 v1
      ↓
Implement Phase 16 v2
      ↓
Architectural Tests
      ↓
Adversarial Semantic Audit ← Freeze Gate ✅
      ↓
Phase 16.5 ← IntrinsicId dispatch ✅ COMPLETE
      ↓
Phase 17 ← Semantic audit (pending)
```

---

## S. Deferred Capabilities

These are out of scope for Phase 16 v2:

- Type construction at runtime
- Runtime RTTI embedding (type_info, type_of structs in stdlib)
- Async/await in comptime
- IO effects in comptime
- Compiler-state mutation
- Advanced reflection (custom attributes, etc.)

Each can be addressed in a future phase with its own design contract.

### Phase 16.5 Dependency

`type_info` and `type_of` intrinsics require:
- `Instruction::CallIntrinsic` in MVIR
- `TypeInfoStruct` placeholder in `mellis-semantic`
- MVIR interpreter intrinsic dispatch

Phase 16.5 implements the compiler-side infrastructure. Full `TypeInfoStruct` layout in `mellis-mlib` (stdlib) comes in Phase 18+.

---

## T. Design Freeze Checklist

### Design Contract

- [x] Purpose: compile-time execution / validation / reflection
- [x] const vs comptime distinction
- [x] Execution model: Mellis → MVIR → Comptime Interpreter
- [x] Type model: TypeValue = SemanticTypeId
- [x] SemanticTypeId canonicalization contract
- [x] IntrinsicId dispatch (representation-independent)
- [x] Reflection intrinsics: type_of, type_info, size_of, align_of
- [x] Reflection metadata structures
- [x] Capability boundary classification
- [x] Layout model sharing
- [x] Generic instantiation identity
- [x] Owned values semantics
- [x] Borrowing restrictions
- [x] Effects enumeration
- [x] Resource limits
- [x] Macro vs comptime boundary
- [x] TypeValue phase boundary
- [x] Implementation approach (semantic rewrite, opportunistic reuse)
- [x] Freeze gate criteria

### Implementation

- [x] AST interpreter removed (`ComptimeEvaluator` deleted)
- [x] Typechecker offloads to MVIR (no AST eval fallback)
- [x] Intrinsic interception via `#[intrinsic]` attribute
- [x] MVIR interpreter purity checking
- [x] MVIR interpreter TypeValue representation
- [x] Unit tests pass (`cargo test -p mellis-mvir`, `cargo test -p mellis-semantic`)

### Freeze Gate

- [x] Adversarial audit: verify Mellis → MVIR → Comptime pipeline
- [x] Confirm no AST evaluation paths remain
- [x] Confirm no hardcoded builtin-name dispatch
- [x] Confirm all intrinsics use IntrinsicId dispatch ✅ FIXED in Phase 16.5
- [x] Confirm `type_info`/`type_of` correctly deferred
- [x] **Phase 16.5**: IntrinsicId dispatch fixed

#### Phase 16.5 Completed

All deviations from Phase 16 v2 design contract have been resolved:

1. **MVIR `CallIntrinsic`**: New instruction `Instruction::CallIntrinsic { kind: IntrinsicKind, args }`
2. **Generator wiring**: MVIR generator reads `expr_intrinsics` map and emits `CallIntrinsic`
3. **Interpreter dispatch**: `MvirInterpreter::eval_intrinsic()` handles all intrinsics
4. **`type_of`, `type_info`, `size_of`, `align_of`**: All implemented via `CallIntrinsic`

**See:** [docs/Phase16.5_IntrinsicDispatch_Plan.md](docs/Phase16.5_IntrinsicDispatch_Plan.md)
