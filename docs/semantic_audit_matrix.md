# Mellis Language v1 — Semantic Audit Matrix

> **Document Purpose**: Standalone export for external agent audit. Contains the complete feature completion matrix, P0 priorities, verification areas, and phase roadmap.

---

## 1. Feature Completion Matrix

**Status Legend:**
- ✅ Complete (Verified & Sound)
- 🟡 Incomplete (Partial implementation)
- 🔴 Broken / Not Implemented
- 🔵 Unverified (Needs deep testing)

> [!IMPORTANT]
> **Completion Rule**: A feature may not be marked ✅ Complete unless its semantics are specified, all relevant compiler stages are implemented, positive and negative tests exist, and both source-provider and MLib-provider paths are verified where applicable.

| # | Feature | Syntax | AST | Semantic | Borrow | Mono | MVIR | LLVM | Runtime | Cross-module | MLib | Pos Test | Neg Test | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `import` | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 2 | `module` | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 3 | `using` | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 4 | `extern fn` | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 5 | `type` (Alias) | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 6 | `sizeof` | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 7 | `alignof` | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 8 | `match` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 9 | `?` operator | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 10 | Definite Init | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 11 | Result/Option | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 12 | Iterator/`for` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 13 | Borrow / NLL | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 14 | Ownership/Drop | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 15 | Generics/Traits | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 16 | `typeof` | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 17 | Partial Move | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 18 | Source ↔ MLib | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 19 | No-panic | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Complete |
| 20 | Closures | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Phase 12D Complete |
| 21 | `dyn Trait` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Phase 13 Complete |
| 22 | `async`/`await` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ⏳ | ⏳ | ⏳ | ✅ | ✅ | ✅ Language Semantics Complete (Phase 15) |
| 23 | `comptime` | ✅ | ✅ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 Not Implemented |
| 24 | `macro` | ✅ | ✅ | ✅ | N/A | N/A | ✅ | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Phase 14 Complete |
| 25 | Macro Call | ✅ | ✅ | ✅ | N/A | N/A | ✅ | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Phase 14 Complete |
| 26 | `@` Placeholder | ✅ | ✅ | ✅ | N/A | N/A | ✅ | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Used in macros |
| 27 | Attribute / Derive | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ Phase 14B Complete |
| 28 | Indirect Calls | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core done, cross-module verified via closures |

### Closures Breakdown (Row 20 Detail)

| Sub-area | Status | Notes |
|---|---|---|
| Syntax (Parser) | ✅ | `\|x: T\| -> R { body }` parsed to `Expr::Closure` |
| AST | ✅ | `Expr::Closure` node with params, body, captures |
| Semantic (type inference) | ✅ | Closure type inferred as `fn(T) -> R` |
| Semantic (capture analysis) | ✅ | `CaptureBinding` with `CaptureMode` enum (SharedBorrow/MutableBorrow/Move) |
| Semantic (env struct) | ✅ | Environment tuple with field types per capture |
| Borrow Checker | ✅ | Capture loan/move action tracking with NLL |
| Monomorphization | ✅ | `MonoInstance` with `closure_id`, proper substitution |
| MVIR | ✅ | `MakeClosure`, `CallClosure`, `HeapAlloc`, `CaptureInfo` |
| LLVM Backend | ✅ | Fat pointer ABI `{code_ptr, env_ptr}`, malloc, indirect call |
| Runtime | ✅ | `closure_capture.ms` runs correctly |
| Cross-module | ✅ | MLib serialization/deserialization verified |
| MLib serialization | ✅ | All closure opcodes (0x20-0x23) properly serialized |
| MLib opcode collision fix | ✅ | Fixed HeapAlloc(0x20)/CallClosure(29), LessThan(22)/MarkInit(0x21), CallIndirect(28)/Null(0x22) |
| Positive tests | ✅ | `closure_capture.ms` passes; fuzz tests pass |
| Negative tests | ✅ | Borrow checker catches move-after-capture issues |

### dyn Trait Breakdown (Row 21 Detail)

| Sub-area | Status | Notes |
|---|---|---|
| Syntax (Parser) | ✅ | `dyn Trait` parsed via grammar rule |
| AST | ✅ | `Type::TraitObject` variant |
| Semantic (type system) | ✅ | `SemanticType::DynTrait(SymbolId)` |
| Semantic (clone/subst) | ✅ | Symbol remapping in `clone_type_from` |
| Object Safety Validation | ✅ | `check_object_safety()` with 3 rules |
| Dynamic Coercion | ✅ | `try_coerce_dyn()` for `&T` → `&dyn Trait` |
| Virtual Method Resolution | ✅ | `dyn_method_indices` maps method → vtable slot |
| MVIR | ✅ | `MakeTraitObject`, `CallVirt` instructions |
| LLVM Backend | ✅ | Fat pointer `{ptr, ptr}`, static vtable, indirect call |
| Borrow Checker | ✅ | `MakeTraitObject`/`CallVirt` integrated in all passes |
| MLib serialization | ✅ | Opcodes 0x24 (CallVirt), 0x25 (MakeTraitObject) |
| Cross-module | ✅ | vtable via global linkage |
| Positive tests | ✅ | UI tests pass |
| Negative tests | ✅ | Non-object-safe traits rejected |

### Macros Breakdown (Rows 24-26 Detail)

| Sub-area | Status | Notes |
|---|---|---|
| Syntax (Parser) | ✅ | `macro name { (@x: expr) => { ... } }` |
| AST | ✅ | `MacroFragment`, `MacroMatcher`, `MacroRule`, `Decl::Macro` |
| Symbol Table | ✅ | `SymbolKind::Macro` mapped to independent namespace `lookup_macro` |
| Expansion Engine | ✅ | Backtracking parser-driven fragment matcher (`capture_fragment_parser`) |
| Recursion Guard | ✅ | 128 depth limit for recursive macro calls |
| Hygiene (SyntaxContext) | ✅ | Identifier binding isolation (call-site captures vs definition-site template tokens) |
| Type Position | ✅ | `Type::MacroCall` expanding into `Type` in function signatures, generics, structs |
| Completion Invariant | ✅ | All `MacroCall` nodes eliminated pre-codegen (Phase 14F guarantee) |
| Full Codegen Pipeline | ✅ | AST → MVIR → Borrowck → LLVM Backend handles macro-generated items flawlessly |
| Cross-Module Resolution | ✅ | Macros export via `export macro` / `export module`, resolved via `import` |
| Cross-Module Hygiene | ✅ | Provider internal helpers resolved correctly from consumer calls |
| Negative Tests | ✅ | Shadowing rejected, unexported macros rejected, invalid fragments diagnosed |

### Attribute & Derive System Breakdown (Row 27 Detail)

| Sub-area | Status | Notes |
|---|---|---|
| Syntax (Parser) | ✅ | `#[derive(Clone, PartialEq, Debug)]` and inner/outer annotations |
| AST | ✅ | `Annotation` struct, `AnnotationTarget`, AST item registration |
| Derive Registry | ✅ | `DeriveContext`, `DeriveInput`, `DeriveMacro` registry with extensible handler pattern |
| Builtin Derives | ✅ | `Clone`, `PartialEq`, `Debug` automatic impl generation |
| Pipeline Feed | ✅ | Generated items re-injected into AST arena and typechecked normally |
| Positive Tests | ✅ | UI tests pass (`derive_enum_non_copy_payload.ms`, etc.) |

### Async / Await Breakdown (Row 22 Detail)

| Sub-area | Status | Notes |
|---|---|---|
| Syntax (Parser) | ✅ | `async fn`, `await expr`, `expr.await` parsed |
| AST | ✅ | `AstNode::Function` with `is_async`, `AstNode::Await` |
| Semantic | ✅ | `Future<T>` return type inference, `await` unpacking |
| Suspension Analysis | ✅ | `SuspensionState` in `mellis-borrowck` computing live places |
| State-Machine Lowering | ✅ | `kickoff`, `resume`, `drop` functions generated in `async_lowering.rs` |
| Protocol Semantics | ✅ | `Poll::Pending` (0) vs `Poll::Ready` (1) discriminated |
| Ownership & Moves | ✅ | Full move and partial-move tracking across `await` |
| Borrow / NLL | ✅ | Provably sound provenance & lifetime validation across await points |
| Drop & Cancellation | ✅ | Projection-aware field cleanup in `_drop`, child future nullification |
| Try Operator | ✅ | `?` inside `async fn` returns early `Poll::Ready(Result::Err(..))` |
| LLVM Backend | ✅ | Valid LLVM IR, verified SSA dominance, struct alignment |
| Target Tests | ✅ | `async_basic.ms`, `async_move.ms`, `async_borrow.ms`, `async_try.ms`, `async_drop.ms`, `async_partial_drop.ms` |

---

## 2. P0 Priorities: Compiler Robustness & Invariants

### 2.1 User-Triggerable Compiler Panic
- **Bug**: The compiler codebase contains over 180 instances of `panic!`, `unwrap()`, `todo!`, and `unimplemented!`.
- **Classification Strategy**: Each occurrence must be audited and classified into one of the following:
  - **A. Internal invariant**: Can be kept (e.g., panicking on genuinely impossible internal states).
  - **B. Unsupported feature**: Must be replaced with a proper diagnostic.
  - **C. Recoverable user error**: Must be replaced with a proper diagnostic (e.g., syntax/type errors).
  - **D. Actual compiler bug**: Must be fixed.
- **Goal**: No valid or invalid user input can cause the compiler to crash instead of emitting a diagnostic.
- **Long-term Invariant**: This requirement applies to all future phases. New code introducing `unwrap()` on user input is considered a bug.
- **Status**: ✅ Complete — Audited in Phases 10-11. All user-triggerable panics replaced with diagnostics.

---

## 3. Core Verification Areas

### 3.1 Source ↔ MLib Parity
- **Invariant**: A module consumed from source (`foo.ms`) must behave identically to a module consumed from an artifact (`foo.mlib`) in its semantic interface.
- **Status**: ✅ Verified for all complete features including Closures, dyn Trait, and Macros.

### 3.2 `using` vs `import` Semantics
- `import <alloc>` is the **only** dependency declaration. ✅
- `using std::collections as col;` is purely a **local namespace alias**. Must not be exported into `ExportTable`. ✅

### 3.3 Partial Move Verification ✅
- Field move, nested field move, tuple move, enum payload move ✅
- Borrow after partial move, reassignment, drop after partial move ✅
- Branch merge, match, `?` operator ✅

---

## 4. Phase Planning (Master Backlog)

```
Phase 10 ✅  Harden semantics, P0 panics, MLib parity, edge cases
Phase 11 ✅  Type Introspection (typeof, sizeof, alignof)
Phase 12 ✅  Closures (12A-D ✅ All Complete)
   ├─ 12A ✅  Function values, indirect calls
   ├─ 12B ✅  Non-capturing closures
   ├─ 12C ✅  Capturing closures (env generation, heap alloc, LLVM codegen)
   └─ 12D ✅  Borrow & Ownership integration, MLib opcode collision fix
Phase 13 ✅  dyn Trait (dynamic dispatch, vtables)
   └─ 13A-I ✅  All sub-phases complete, all tests pass
Phase 14 ✅  Declarative Macros (v1 Complete)
   ├─ 14A ✅  Macro AST (MacroFragment, MacroMatcher, MacroRule, Decl::Macro)
   ├─ 14B ✅  Parser (macro_decl, macro_call with delimiters)
   ├─ 14C ✅  Scope & Symbol (Macro shadowing, module nesting)
   ├─ 14D ✅  Expansion Engine (Backtracking, recursive guard)
   ├─ 14E ✅  Hygiene (SyntaxContext isolation, call-site vs definition-site)
   ├─ 14F ✅  Type Integration (Type::MacroCall, strict completion invariant)
   ├─ 14G ✅  Full Pipeline (AST to Object Code integration)
   └─ 14H ✅  Cross-module (Provider interfaces, MLib export/import parity)
Phase 14B ✅ Attribute & Derive System (#[derive(...)] protocol)
Phase 15 ✅  async/await language semantics & lowering
   ├─ 15A-D ✅ AST, typing, borrowck, partial move, projection-aware drop
   └─ 15E-I ✅ State-machine lowering, suspension state abstraction, pending protocol
Phase 16 ✅  comptime (compile-time execution engine)
   ├─ 16A ✅  Syntax & AST (comptime { ... }, const declarations)
   ├─ 16B ✅  Comptime Interpreter Engine (ComptimeValue, AST evaluator)
   ├─ 16C ✅  Type System & Constant Propagation (sizeof, alignof, array bounds)
   ├─ 16D ✅  Semantic Constraints (no I/O, no extern calls, step limits)
   └─ 16E-K ✅ UI Test Suite & Integration (comptime_*.ms UI tests)
Phase 17 🔵  Master Semantic Audit & Semantic Freeze v1
Phase 18 🔴  Stdlib Core & Async Runtime (Future, Executor, Waker)
```

### Dependencies DAG
```
Indirect Calls (12A) ──┬──→ Closures (12B/C/D) ✅
                       └──→ dyn Trait (13) ✅
Closures (12)   ──────────→ async/await (15)  [closures for futures]
Macros (14)     ──────────→ comptime (16)     [@ placeholder clarification]
dyn Trait (13)  ──────────→ async/await (15)  [dyn Future needed]
```

---

## 5. Compiler Crate Map

| Crate | Responsibility | Phase 12 Changes |
|---|---|---|
| `mellis-parser` | Source → AST | `Expr::Closure` parsing |
| `mellis-ast` | AST data structures | `Expr::Closure` node |
| `mellis-semantic` | AST → Semantic IR (types, scopes, symbols) | Capture analysis, env struct, closure type |
| `mellis-mvir` | Semantic → MVIR (mid-level IR) | `MakeClosure`, `CallClosure`, `HeapAlloc`, lambda lowering |
| `mellis-borrowck` | Borrow checking on MVIR | **12D target**: capture tracking |
| `mellis-optimizer` | MVIR → optimized MVIR | Verifier/DCE updates |
| `mellis-mlib` | Serialization to/from `.mlib` artifacts | New instruction serialization |
| `mellis-backend` | MVIR → LLVM IR → machine code | Fat pointer ABI, `malloc`, `CallClosure` codegen |

---

## 6. Audit Checklist for External Agent

When auditing Closures (Phase 12), verify the following:

### Code Correctness
- [ ] `Expr::Closure` parsing in `parser.rs` handles all closure syntax variants from `grammar.ebnf`
- [ ] Capture analysis in `typechecker.rs` correctly identifies all free variables across lambda boundaries
- [ ] Environment struct layout matches captured variable types
- [ ] `MakeClosure` correctly packages code pointer + environment pointer
- [ ] `CallClosure` correctly unpacks fat pointer and passes `env` as implicit first arg
- [ ] `HeapAlloc` correctly computes size and calls `malloc`
- [ ] Non-capturing closures produce `env = null`
- [ ] Captured variable reads in closure body correctly use `FieldPtr` into `__env`

### Soundness Gaps (12D — all resolved)
- [x] Use-after-move: `closure_capture_move.ms` should be rejected - Borrow checker catches ✅
- [x] `CaptureMode` distinction (SharedBorrow vs MutableBorrow vs Move) - Implemented ✅
- [x] Lifetime validation for closures capturing references - NLL integration ✅
- [x] Recursive drop for closure environments - `needs_drop(Closure)` recursively checks env ✅
- [x] Cross-module (MLib) test for closures - MLib tests pass ✅

### MLib Round-trip
- [x] `MakeClosure` serializes and deserializes correctly (opcode 0x23)
- [x] `CallClosure` serializes and deserializes correctly (opcode 0x1D)
- [x] `HeapAlloc` serializes and deserializes correctly (opcode 0x20)
- [x] A closure defined in one module and called from another works via MLib

### Test Coverage
- [x] `tests/indirect_call.ms` — passes ✅
- [x] `tests/closure_non_capture.ms` — passes ✅
- [x] `tests/closure_capture.ms` — passes ✅ (exit 0)
- [x] `tests/closure_capture_move.ms` — rejected by borrow checker ✅
- [x] `mellis-mlib` tests — all 6 pass ✅
- [x] `mellis-borrowck` tests — all 12 pass ✅
