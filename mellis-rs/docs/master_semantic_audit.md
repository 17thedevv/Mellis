# Master Semantic Audit

**Overall Freeze Decision**: `NOT READY`

This is the living document for the zero-tolerance Master Semantic Audit of the Mellis compiler.

---

## Batch 1: Frontend & Core Semantics

### A. Grammar & AST Coverage
**Coverage of AST constructs to semantic lowering.**

- **Type::Lifetime**
  - **Status**: `[FAIL]`
  - **Evidence**: `mellis-ast/src/ty.rs` defines `Type::Lifetime(Span)`, but `lower_type` in `typechecker.rs` (lines 712-790) omits this variant from its `match ast_ty`, causing it to silently fall back to `_ => self.ctx.types.intern(SemanticType::Error)`.
  - **Triage**: P1 (Semantic feature incomplete). If lifetimes are part of the grammar, they must be correctly lowered or explicitly rejected with a clean diagnostic.

- **Type::Named (Generics Instantiation)**
  - **Status**: `[FAIL]` / `[UNSOUND]`
  - **Evidence**: `lower_type` processes `Type::Named` (lines 739-768) but entirely ignores `generic_args: Vec<TypeId>` (except for a hardcoded hack for `Future`). `lower_type` returns the uninstantiated base generic type. 
  - **Triage**: P0. Explicit type instantiations (e.g., `Box<i32>`) are parsed into AST but discarded by the semantic layer.

- **Expr::Call / Expr::MethodCall / Expr::StructInit / Expr::Identifier (Generics Instantiation)**
  - **Status**: `[FAIL]` / `[UNSOUND]`
  - **Evidence**: These expressions in `typecheck_expr` (e.g., line 1228, line 1405) ignore `generic_args: Vec<TypeId>`. For example, `StructInit` only unifies the values against the generic struct's *uninstantiated holes* (inference variables or `GenericParam`), meaning `Box::<i32> { ... }` is semantically identical to `Box { ... }` and both bypass concrete generic typing.
  - **Triage**: P0. Value-level generic arguments are discarded. 

- **Expr::MacroCall / Stmt::MacroCall**
  - **Status**: `[FAIL]`
  - **Evidence**: `Expr::MacroCall` and `Stmt::MacroCall` are missing from `typecheck_expr` and `typecheck_stmt` respectively. If an unexpanded macro call slips through to the typechecker, it evaluates silently to `Error` without a dedicated diagnostic (unlike `Type::MacroCall` which does emit an ICE).
  - **Triage**: P1.

- **Stmt::Break / Stmt::Continue**
  - **Status**: `[FAIL]` / `[UNSOUND]`
  - **Evidence**: `resolver.rs` and `typechecker.rs` completely ignore these variants (`_ => {}`). Furthermore, `mellis-mvir/src/generator.rs` silently drops them if they appear outside a loop (`if let Some(...) = self.loop_scopes...` fails, emitting no instructions).
  - **Impact**: A `break;` outside a loop is syntactically valid, semantically ignored, and compiles to a no-op at runtime without emitting any error.
  - **Triage**: P0. Complete failure of control flow validation.

- **Expr::Try**
  - **Status**: `[FAIL]` / `[UNSOUND]`
  - **Evidence**: In `typechecker.rs`, `Expr::Try` is typed identically to its inner expression: `self.typecheck_expr(e)`. In `mellis-mvir/src/generator.rs`, it is generated as `self.generate_expr(inner)`.
  - **Impact**: The `try` keyword (e.g. `foo()?`) is merely syntactic sugar that does nothing. It does not unwrap `Result<T, E>`, it does not early return on error.
  - **Triage**: P0.

### B. Name Resolution & Scope
**Lexical scoping, symbol registration, and generics scoping.**

- **Struct / Enum / Trait Generic Parameters**
  - **Status**: `[FAIL]` / `[UNSOUND]`
  - **Evidence**: In `resolver.rs` `declare_item` (lines 271-284), `Decl::Struct` (as well as `Enum` and `Trait`) completely ignores `generic_params`. It fails to register `TypeParam` symbols for its generics. 
  - **Impact**: When `typechecker.rs` `populate_types` processes a generic struct (e.g., `struct Box<T> { value: T }`), it looks up `T` in the outer scope, fails, and silently assigns a fresh inference variable (`?0`) to the field. Consequently, generic structs in Mellis are currently completely untyped and structural generics are an illusion.
  - **Triage**: P0. Fundamental flaw in generic data structures.

- **Struct / Enum Inner Scopes**
  - **Status**: `[FAIL]`
  - **Evidence**: `resolver.rs` creates an `inner_scope` for `Module` and `Function` but omits it for `Struct`, `Enum`, and `Trait`.
  - **Triage**: P0.

### C. Type System
**Type coercion, inference, equality.**

- **Generics Subsitution & Unification**
  - **Status**: `[UNSOUND]`
  - **Evidence**: Because structs are typed with uninstantiated generic holes (`GenericParam` or inference vars), and because `unify()` permits `GenericParam` to unify with anything without actually performing semantic substitution across the struct, type checking against generic structs succeeds trivially but vacuously. 
  - **Triage**: P0.

- **Dynamic Trait Coercion (try_coerce_dyn)**
  - **Status**: `[FAIL]`
  - **Evidence**: `try_coerce_dyn` hardcodes the concrete type check to `SemanticType::Struct` (line 696). 
  - **Impact**: It is impossible to coerce an `Enum`, `Primitive`, or any other type to a `dyn Trait`, even if they implement the trait.
  - **Triage**: P1.

- **Recursive Type Checking**
  - **Status**: `[FAIL]`
  - **Evidence**: `typechecker.rs` performs no checks for infinite-sized types (e.g., `struct Node { value: i32, next: Node }`).
  - **Impact**: Such types parse and typecheck correctly but will cause LLVM size calculation to hang or crash in the backend.
  - **Triage**: P1.

---

## Batch 2: Deep Semantics & Soundness Boundary

- [x] Phase 1: Semantic Type Migration & Identity (Completed)
- [x] Phase 2: Trait Implementation Constraints (Completed)
- [x] Phase 3: Lifetime & Borrow Checker Audit (Completed)
- [x] Phase 4: Comptime Evaluation Soundness (Completed)
- [x] Phase 5: Dependency Graph & Fallback Elimination (Completed)
- [x] Phase 6: Visibility & Access Control Enforcement (Completed)
  - **Issues found**: `Visibility::Private` access bounds were not fully enforced in path resolution for aliases and deeply nested `Expr::Member`. Additionally, a major scoping bug existed in `resolver.rs` where `Decl::Module` inadvertently mutated `self.current_scope` inside `self.enter_scope()`, causing all subsequent items (like `main`) to be incorrectly evaluated as descendants of the module scope.
  - **Fixes applied**: Rewrote `is_accessible` check and threaded `self.current_scope` to all lookup points across `typechecker.rs` and `resolver.rs`. Fixed the module scope enter/exit leak to correctly restore parent scope context.
- [ ] Phase 7: Semantic Soundness Testing (Pending)
- [ ] Phase 8: Mellis Standard Library Integration (Pending)
- [ ] Phase 9: Self-Hosted Compiler Bootstrapping Audit (Pending)

### R. Cross-cutting Silent Fallbacks & Error Containment
**Grep sweep across `crates/` for `_ => 0`, `return false`, `_ => None`, `SemanticType::Error`.**

A total of **62 silent fallbacks** were identified. Below are the most critical `[FAIL] UNSOUND (P0)` fallbacks where the compiler silently recovers with invalid semantics instead of generating a diagnostic.

- **`needs_drop` ignoring generic parameters and dyn traits**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: 
    1. Source: `crates/mellis-semantic/src/lib.rs:124` (`_ => false`)
    2. Soundness: `SemanticType::DynTrait` and `SemanticType::GenericParam` hit this fallback and return `false` (no drop needed). A trait object or generic variable holding a heap allocation will silently leak memory because drop glue is never emitted.
    3. Positive Test: `struct S { a: i32 }` (returns `false`, correct).
    4. Negative Test: `fn foo<T>(x: T) { }` (returns `false`, incorrect if `T` is `String`).
    5. Cross-feature: `needs_drop` × `GenericParam` / `DynTrait`.

- **Trait Bounds Checking ignoring Enums**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**:
    1. Source: `crates/mellis-semantic/src/typechecker.rs:538` (`_ => None`)
    2. Soundness: When checking trait bounds for a generic parameter, `concrete_struct_sym` only extracts the symbol if the type is `SemanticType::Struct`. For `SemanticType::Enum`, it hits the fallback `_ => None` and bypasses trait bound verification entirely.
    3. Positive Test: `struct S` bound checks correctly.
    4. Negative Test: `enum E` bypasses bounds checks.

- **`is_copy_type` ignoring Generics / Dyn Traits**
  - **Status**: `[FAIL] INTERNAL RECOVERY (P0)`
  - **Evidence**:
    1. Source: `crates/mellis-mvir/src/interp.rs:172` (`_ => false`)
    2. Soundness: While safely returning `false` for structs, returning `false` blindly for `GenericParam` means generic values are always moved in comptime/MVIR evaluation. If a generic bound requires `Copy`, this fallback contradicts it, leading to mismatched move/copy semantics.

- **Unrecognized Literals typed as Error without Diagnostic**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**:
    1. Source: `crates/mellis-semantic/src/typechecker.rs:1166` (`_ => SemanticType::Error`)
    2. Soundness: If an unexpected literal token reaches typechecking, it returns `Error` without pushing a diagnostic to `self.ctx.diagnostics`. The error is swallowed, creating an unexecutable MVIR block later.

### D. Declaration & Definition Semantics
**Audit of definition consistency, duplication, and bounding.**

- **Duplicate Struct Fields & Enum Variants**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: `resolver.rs` ignores struct fields and enum variants during `declare_item`. `typechecker.rs` merely iterates over fields and pushes them into `SemanticType::Struct` / `SemanticType::Enum` without any uniqueness verification. `struct S { a: i32, a: f64 }` is considered a perfectly valid type.

- **Duplicate Trait Implementations Overwrite Prior Ones**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: `typechecker.rs` registers implementations via `self.ctx.tables.trait_impls.insert(ImplKey, decl_id)`. The `ImplKey` contains the trait `SymbolId` and the struct `SymbolId` but crucially omits any generic type arguments or uniqueness checks. Thus `impl Foo<i32> for S` and `impl Foo<f64> for S` have the exact same `ImplKey` and will silently overwrite each other, causing methods from the first impl to be lost. Multiple inherent `impl S { ... }` blocks also overwrite each other.

### E. Ownership / Borrow / Initialization / Drop
**Definite initialization and lifetime tracking.**

- **Definite Initialization**
  - **Status**: `[PASS]`
  - **Evidence**: `mellis-borrowck/src/move_analysis.rs` uses a robust forward dataflow engine over the MVIR. Local variables start as `MoveState::Uninitialized`. Assignments correctly transition them to `MoveState::Live`. Control flow joins (e.g. `Live` from `if` and `Uninitialized` from `else`) correctly merge to `MoveState::ConditionallyMoved`. Any usage of `Uninitialized` or `ConditionallyMoved` successfully emits a diagnostic.

### F. Control Flow / CFG / Divergence
**Block expression returns, early returns, break/continue.**

- **Silent Swallow of `return` Type Mismatches**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `typechecker.rs:1135`, the code to check `return` expressions is `let _ = self.unify(expected_ty, val_ty);`. The result of `unify` is ignored, and no diagnostic is pushed if unification fails. Therefore, returning a string from an `i32` function silently compiles with completely mismatched MVIR return types, violating memory safety.

- **`break` / `continue` Semantic Blackhole**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: As identified in Batch 1, they are ignored by the semantic layer and the MVIR generator drops them if outside a loop. This breaks CFG generation entirely.

### G. Pattern Matching / Destructuring
**Exhaustiveness, structural unpacking, binding types.**

- **Match Arms Ignore Unification (Type Blackhole)**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `typechecker.rs:1673` (`Expr::Match`), the result type of the entire match expression is unilaterally assigned to the type of the **first arm (`if i == 0`)**. Subsequent arms are never unified against `result_ty`. This means `match x { 1 => "str", 2 => 42 }` typechecks as a string, completely ignoring the `i32` in the second arm, which will crash the backend.

- **Struct Pattern Silent Fallbacks**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `typechecker.rs:1011` (`typecheck_pattern` for `Struct`), if a pattern specifies a field that does NOT exist in the struct declaration, the loop fails to find it, leaves its type as an unconstrained inference variable, and emits NO diagnostic. Furthermore, there is zero check for missing fields when `has_rest` (`..`) is omitted. `S { fake_field: val }` compiles silently.

- **Enum Pattern Arity Bypass**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `typechecker.rs:1055` (`typecheck_pattern` for `Enum`), the code iterates over the provided pattern fields and zips them with the variant's payload types. If the pattern provides *too many* fields, the fallback `unwrap_or_else(|| self.ctx.types.new_inference_var())` creates unconstrained types for the excess variables without error. If it provides *too few* fields, it just stops iterating without error. `Enum::A(1, 2, 3)` matches `Enum::A(i32)`.

### H. Generics / Substitution / Trait Bounds (Deep Dive)
**Generic instantiation, bounds on types, bounds on methods.**

### Phase 5: Traits & Bounds
**Status: COMPLETE**

- **Struct Initialization Ignores Trait Bounds**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: `check_bounds_for_call` is invoked *only* for `Expr::Call` and `Expr::MethodCall`. During `Expr::StructInit` (line 1405), the compiler unifies the fields but completely skips verifying any trait bounds defined on the struct's generic parameters. `struct Box<T: Display> { val: T }` can be instantiated with an unprintable type without error.

### I. Trait / Impl Consistency
**Method signature compliance and completeness.**

### J. Closures
**Capture analysis, environment types, mutability bounds.**

- **Closure Capture & Move Semantics**
  - **Status**: `[PASS]` (with caveat from F)
  - **Evidence**: `typechecker.rs` properly categorizes captures as `SharedBorrow`, `MutableBorrow`, or `Move`. `move_analysis.rs` accurately enforces `MoveState::Moved` for `CaptureMode::Move`, preventing use-after-move. `borrow_analysis.rs` prevents closures capturing local borrows from being returned. However, the generic `return` type mismatch bypass (Section F) also affects `return` statements inside closures.

### K. Async / Await
**Future types, async blocks, generator lowering.**

- **Async / Await Type Enforcement**
  - **Status**: `[PASS]`
  - **Evidence**: `typecheck_function` correctly wraps the `return_type` in `SemanticType::Future` if `is_async` is true. `Expr::Await` correctly enforces that it must reside within an `async` function and safely extracts the inner type from `SemanticType::Future`.

### L. Comptime
**Compile-time evaluation blocks, const evaluation.**

- **Comptime Statements Ignored; Comptime Expressions Leak to Runtime**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: `typecheck_stmt` in `typechecker.rs:1147` simply ignores `Stmt::Comptime` (`_ => {}`), causing comptime statements to be completely swallowed. For `Expr::Comptime`, while it calls `eval_comptime_stmt`, `generator.rs:1321` lowers `Expr::Comptime` directly to runtime MVIR via `self.generate_block_expr(body)`. This defeats the entire purpose of `comptime` as the block executes at runtime regardless.

### M. Macro / Hygiene
**AST expansion, recursion limits, token hygiene.**

- **Macro Diagnostics Replaced By Empty Tokens**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: (From Batch 1) `macro_engine.rs` returns `Vec::new()` (empty token stream) upon transcription errors instead of pushing diagnostics, causing malformed macros to silently disappear from the AST.
- **Recursion & Hygiene**
  - **Status**: `[PASS]`
  - **Evidence**: Macros correctly implement a recursion limit of 128 (`recursion_depth`) and enforce hygiene by modifying token `SyntaxContext` (`t.span.ctxt = SyntaxContext(expansion_id);`).

### N. Modules / Import / Using
**Namespace resolution and visibility.**

- **Phantom Imports & Usings**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `resolver.rs`, when processing `Decl::Using` and `Decl::Import`, if the target scope or provider does not exist (`target_scope_opt == None`), the compiler silently creates an empty alias symbol without pushing any diagnostic. `using fake::path as alias;` and `import <nonexistent>;` compile silently, breaking the module boundary completely.

### O. MVIR Semantic Preservation
**Generics instantiation and inference resolution.**

- **Un-substituted Types Leak to MVIR**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: `mono.rs` correctly uses `Substitution` for known generics. However, if type inference fails or a pattern omits fields (as found in Section G), variables are left as `SemanticType::InferenceVar`. The MVIR `generator.rs` faithfully extracts these un-substituted `InferenceVar` and `GenericParam` types and passes them down to the backend as valid types.

### P. LLVM Backend Soundness
**Code generation and memory layout.**

- **Universal Type Coercion (`_ => i32`)**
  - **Status**: `[FAIL] UNSOUND (P0)`
  - **Evidence**: In `llvm_codegen.rs:205`, the `map_type` function explicitly matches unresolved or unsupported types (`InferenceVar`, `GenericParam`, etc.) with a catch-all fallback: `_ => Ok(self.context.i32_type().into())`. This means any unresolved type in the semantic layer silently compiles into a 32-bit integer at the LLVM level, leading to arbitrary memory corruption without any compiler error.
