# LANG-FEATURE-MATURITY-01: Phase 0 Ground-Truth Capability & Pipeline Audit

**Milestone**: LANG-FEATURE-MATURITY-01  
**Status**: COMPLETE & VERIFIED ✅  
**Authority**: Luna Language Specification & Compiler Implementation Audit  

---

## 1. Executive Summary

Phase 0 establishes the empirical ground truth for Luna language capabilities prior to constructing hardening and adversarial stress test suites. Each capability is classified into one of four authoritative dispositions:

1. **PROVEN-SUPPORTED**: Verified in parser, semantic analysis, MVIR/interpreter, and code generation. Only these capabilities generate positive executable stress tests.
2. **EXPLICITLY-UNSUPPORTED**: Prohibited by language design, safety invariants, or phase ordering. Generates negative rejection tests asserting authoritative compiler diagnostics.
3. **IMPLEMENTED-BUT-UNSPECIFIED**: Present in code but lacking explicit language contract or consistent cross-module behavior. Hardened with specific regression locks.
4. **LANGUAGE-GAP**: Capability theoretically desirable or planned, but blocked by current compiler architecture or phase ordering. Recorded as a frozen gap without synthetic bypasses.

---

## 2. Capability Matrix & Dispositions

| Capability Domain | Specific Feature / Contract | Status / Disposition | AST / Semantic Location | Positive Oracle / Rejection Mode |
|:---|:---|:---|:---|:---|
| **Macros** | Metavariables `@x: frag` & `$x: frag` | `PROVEN-SUPPORTED` | `luna-parser/decl.rs:761` | Expands correctly in expr/stmt/type positions |
| **Macros** | Repetitions `*` (zero+), `+` (one+) | `PROVEN-SUPPORTED` | `luna-semantic/macro_engine.rs:828` | Matches sequences, expands repeated fragments |
| **Macros** | Repetition `?` (optional) | `PROVEN-SUPPORTED` | `luna-ast/decl.rs:217`, `parser:784` | Matches zero or one occurrence |
| **Macros** | Fragment kinds: `expr`, `ident`, `ty`, `stmt`, `block`, `item`, `tt`, `pat`, `path`, `meta`, `literal` | `PROVEN-SUPPORTED` | `luna-parser/decl.rs:740` | Fragment matches specified syntactic class |
| **Macros** | Fragment kind: `vis` | `IMPLEMENTED-BUT-UNSPECIFIED` | `luna-semantic/macro_engine.rs:943` | Matches legacy `pub` / empty; Luna `export` requires audit lock |
| **Macros** | Hygiene & lexical scoping | `PROVEN-SUPPORTED` | `macro_engine.rs`, `SyntaxContext` | Caller and macro local variable isolation |
| **Macros** | Recursion depth limit (128) | `EXPLICITLY-UNSUPPORTED` | `macro_engine.rs:402, 554, 654` | Rejection: `"recursion limit reached while expanding macro"` |
| **Macros** | Cross-module transport via source | `PROVEN-SUPPORTED` | `cross_module_macro_tests.rs` | `export macro` accessible via provider / qualified path |
| **Macros** | Cross-module transport via `.llib` | `PROVEN-SUPPORTED` | `luna-llib/writer.rs:38`, `reader.rs:736` | AstInterface preserves `Decl::Macro`, relocated on load |
| **Comptime** | Basic const arithmetic & logic | `PROVEN-SUPPORTED` | `luna-mvir/interp.rs` | Folded at compile-time to constant value |
| **Comptime** | Block evaluation `comptime { ... }` | `PROVEN-SUPPORTED` | `luna-semantic/typechecker.rs:5483` | Lowers to MVIR, runs in `MvirInterpreter` |
| **Comptime** | Local mutation & loops (`while`) | `PROVEN-SUPPORTED` | `luna-mvir/interp.rs` | Imperative evaluation within pure comptime sandbox |
| **Comptime** | Step limit (1,000,000) & recursion limit (512) | `EXPLICITLY-UNSUPPORTED` | `interp.rs:188`, `interp.rs:222` | Rejection: `"step limit exceeded"` / `"recursion limit exceeded"` |
| **Comptime** | Memory safety: Leak & escape detection | `EXPLICITLY-UNSUPPORTED` | `interp.rs:320`, `value.rs:118` | Rejection: `"E_COMPTIME_RESOURCE_LEAK"` / `"escape error"` |
| **Comptime** | Side-effect isolation (IO, extern, await) | `EXPLICITLY-UNSUPPORTED` | `interp.rs:150`, `const_eval.rs:116` | Rejection: `"forbidden side-effect in comptime"` |
| **Comptime** | Array length expression `[T; N]` | `PROVEN-SUPPORTED` | `typechecker.rs:2613` | Type-independent comptime expr sizes array |
| **Comptime** | Comptime structs, tuples, enums | `PROVEN-SUPPORTED` | `comptime/value.rs:70` | Composite values computed and decomposed at comptime |
| **Unsafe / FFI** | Raw pointer types `*T` and `*rw T` | `PROVEN-SUPPORTED` | `luna-parser/ty.rs:37` | Canonical pointer types; immutable `*T` vs mutable `*rw T` |
| **Unsafe / FFI** | Raw pointer dereference (`*ptr`) | `PROVEN-SUPPORTED` | `typechecker.rs:5433` | Permitted strictly inside `unsafe { ... }` block |
| **Unsafe / FFI** | Raw pointer dereference outside unsafe | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:5439` | Rejection: `"E_UNSAFE_DEREF_OUTSIDE_UNSAFE"` |
| **Unsafe / FFI** | Mutating through immutable `*T` | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:4029, 5443` | Rejection: `"E_CANNOT_MUTATE_IMMUTABLE_POINTER"` |
| **Unsafe / FFI** | Pointer arithmetic (`ptr + n`, `ptr - n`) | `PROVEN-SUPPORTED` | `typechecker.rs:3744` | Permitted inside `unsafe`; element-stride offset |
| **Unsafe / FFI** | Pointer arithmetic outside unsafe | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:3759` | Rejection: `"E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE"` |
| **Unsafe / FFI** | Pointer arithmetic on unsized types (`dyn`, `[T]`) | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:3752` | Rejection: `"E_UNSIZED_POINTER_ARITHMETIC"` |
| **Unsafe / FFI** | Unsafe semantic containment | `PROVEN-SUPPORTED` | `typechecker.rs:2743` | Unsafe permits designated ops; typecheck & borrowck stay active |
| **Unsafe / FFI** | `extern "C" fn` calling convention | `PROVEN-SUPPORTED` | `typechecker.rs:2961`, `llvm/mod.rs` | Primitive arguments and returns across C ABI |
| **Unsafe / FFI** | Safe reference returned by `extern fn` | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:310` | Rejection: `"error[E2030]: NonFfiSafeType ... without explicit contract"` |
| **Unsafe / FFI** | By-value aggregate crossing FFI with pointers | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:356, 369` | Rejection: `"error[E2030]: By-value aggregate containing pointer capability"` |
| **Unsafe / FFI** | Struct crossing FFI without `#[repr(C)]` | `EXPLICITLY-UNSUPPORTED` | `typechecker.rs:285, 330` | Rejection: `"error[E2030]: Struct is not marked with #[repr(C)]"` |
| **Phase Order** | Macro generating `comptime { ... }` | `PROVEN-SUPPORTED` | Driver pipeline | Macro expands at Step 6, Comptime evaluates at Step 8 |
| **Phase Order** | Comptime result driving macro expansion | `EXPLICITLY-UNSUPPORTED` | Architecture invariant | Unsupported by phase order (Macros expand before Typecheck) |
| **Phase Order** | Type-independent comptime in generics | `PROVEN-SUPPORTED` | `typechecker.rs` | Evaluated pre-monomorphization if independent of type params |
| **Phase Order** | Open generic-dependent comptime (`sizeof(T)`) | `LANGUAGE-GAP` | `luna-mvir/interp.rs:72` | Rejected pre-monomorphization: `E_UNRESOLVED_PROJECTION` |

---

## 3. Detailed Domain Audit Findings

### 3.1 Macro & Metaprogramming System
- **Metavariable Prefix**: Parser accepts both `@` (canonical Luna) and `$` (interop).
- **Repetitions**: Supports `*` (zero or more), `+` (one or more), and `?` (zero or one) with optional `,` or `;` separators.
- **Hygiene**: Hygenic identifiers are marked with `SyntaxContext(expansion_id)`. Lookups in `SymbolTable::lookup_with_ctxt` prevent local variable pollution in calling scopes while preserving module-level item access.
- **Provider & Artifact Transport**: Macros are stored in the AST arena as `Decl::Macro`. When writing `.llib` artifacts, `MlibWriter` serializes the AST interface section containing public declarations including macros. On loading, `MlibReader` deserializes and relocates macro declarations, registering them into `macro_decls`.

### 3.2 Const & Comptime Evaluation
- **VM Engine**: Evaluated via `MvirComptimeEngine` executing inside `MvirInterpreter`.
- **Pure Execution Sandbox**: All side effects (`IO`, `Extern`, `Await`) are strictly forbidden inside comptime. Any attempt immediately aborts evaluation with `ComptimeError::ForbiddenSideEffect`.
- **Limits & Resource Tracking**: Step limit defaults to `1,000,000`; call recursion limit defaults to `512`. Heap allocations are tracked; any unallocated memory remaining when the block finishes execution produces `ComptimeError::ResourceLeak`.
- **Diagnostic Authority**: The compiler wraps evaluation failures in `Diagnostic::error("cannot evaluate comptime block: <reason>")`. Negative tests must assert this structured message and category, without inventing artificial numeric error codes.

### 3.3 Unsafe, Raw Pointers & FFI Boundary
- **Canonical Types**: Raw pointer syntax is strictly `*T` (const / immutable) and `*rw T` (mutable).
- **Unsafe Scope**: `unsafe { ... }` or `unsafe fn` sets `is_unsafe_context = true`. It authorizes:
  1. Dereference of raw pointers (`*ptr`).
  2. Pointer arithmetic (`ptr + offset`, `ptr - offset`).
  3. Invocation of `extern "C" fn` or `unsafe fn`.
- **Active Safety Invariants inside Unsafe**:
  - Type checking, type inference, and coherence remain fully enforced.
  - Ownership moves and drop semantics remain active.
  - Safe references (`&T`, `&rw T`) remain strictly checked by the borrow checker.
  - Mutating an immutable raw pointer `*T` is forbidden even inside unsafe (`E_CANNOT_MUTATE_IMMUTABLE_POINTER`).
- **FFI Safety Contract (E2030)**:
  - Raw pointers have no implicit safe lifetime.
  - Returning a safe reference from `extern fn` is prohibited unless covered by an explicit contract.
  - Aggregates crossing FFI must be `#[repr(C)]`.
  - `#[repr(C)]` is a layout contract, not a safety waiver: aggregates containing raw pointer or reference capabilities cannot cross FFI by value.

### 3.4 Phase Ordering & Monomorphization Boundaries
- **Pipeline Progression**:
  $$\text{Lex/Parse} \longrightarrow \text{Import Resolution} \longrightarrow \text{Macro Expansion} \longrightarrow \text{Typecheck / Comptime} \longrightarrow \text{Monomorphization} \longrightarrow \text{Borrowck} \longrightarrow \text{MVIR} \longrightarrow \text{LLVM}$$
- **Macro $\to$ Comptime**: Valid. Macros run in Step 6 and can produce AST nodes containing `comptime { ... }`, which are then evaluated in Step 8.
- **Comptime $\to$ Macro**: Unsupported by design. Because macro expansion completes before typecheck and comptime evaluation, comptime values cannot be used to determine macro repetition counts or select macro patterns.
- **Generic-Dependent Comptime**: Comptime blocks that do not depend on generic type parameters evaluate successfully in the typechecker. Comptime expressions depending on un-monomorphized generic parameters (`T`) encounter `SemanticType::GenericParam` and are rejected with `E_UNRESOLVED_PROJECTION` as a classified `LANGUAGE-GAP`.

---

## 4. Phase 0 Verdict & Gate Clearance

The compiler capability matrix is authoritative, unambiguous, and verified against the actual codebase. All foreign syntaxes (`*const`, `*mut`, synthetic diagnostics) are eliminated.

**Gate Status**: PHASE 0 COMPLETE & CLEARED FOR EXECUTION ✅
