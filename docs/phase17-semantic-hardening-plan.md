# Phase 17F — Semantic Core Hardening Plan

## 1. Root-Cause Analysis for Known Issues

### Issue 1: `Stmt::For` is Accepted but Ignored in MVIR
- **Root Cause**: `mellis-mvir/src/generator.rs` `generate_stmt()` completely lacks a match arm for `Stmt::For`. The `_ => {}` catch-all silently swallows the AST node, generating no instructions.
- **Why it matters**: A core language loop construct does absolutely nothing at runtime.

### Issue 2: LLVM Backend Lowers Unknown Instructions to `0`
- **Root Cause**: `mellis-backend/src/llvm_codegen.rs` uses `_ => Ok(self.context.i32_type().const_zero().into())` in `generate_inst` and `_ => Ok(self.context.i32_type().into())` in `llvm_type()`.
- **Why it matters**: Missing opcodes (like `BoxNew`, `PtrCast`, `SizeOf`) compile successfully but produce `0` (null pointers or 0 values), leading to disastrous, silent miscompilations.

### Issue 3: `Expr::Comptime` Silently Falls Back to `0`
- **Root Cause**: `mellis_mvir::generator::generate_expr_inner()` looks up primitive comptime values (Int, Float, Bool). If the value is a complex type (e.g., Struct) or failed to evaluate, it falls out of the `if let` block into the main `match expr`, which lacks an `Expr::Comptime` arm. It then hits `_ => Operand::Number("0".to_string())`.
- **Why it matters**: `comptime` struct allocations or failures silently yield `0`.

### Issue 4: Unexpanded Macros Silently Yield `0`
- **Root Cause**: If `macro_engine.rs` encounters a macro error, it leaves `Expr::MacroCall` in the AST. The typechecker assigns it `SemanticType::Error`, but the compiler doesn't abort early enough. It reaches MVIR generator and hits the same `_ => 0` fallback.
- **Why it matters**: Macro invocation failures can succeed in producing an executable that just uses `0` in place of the macro.

### Issue 5: Generic Trait Bounds Are Completely Unchecked
- **Root Cause**: `crates/mellis-semantic/src/typechecker.rs` explicitly uses `..` to ignore `generic_params` when matching `Decl::Function` and `Decl::Struct` (except during object safety checks). It never stores or checks generic bounds (`T: Trait`). 
- **Why it matters**: You can pass `i32` to a generic function `fn foo<T: Clone>()` even if `i32` does not implement `Clone`, causing confusing late monomorphization errors instead of semantic errors.

---

## 2. Recommended Architecture & Fixes

**Fix 1: Eliminate All Catch-Alls (`_ => ...`)**
- Remove `_ => 0` in MVIR generation. Any unsupported expression must invoke `unimplemented!("Unsupported expr: {:?}", expr)`.
- Remove `_ => 0` in LLVM Codegen. Return `Err(BackendError::Unimplemented(...))` and abort compilation.

**Fix 2: Lowering `Stmt::For`**
- `Stmt::For` in Mellis acts as a `for_each_stmt` (`for pattern in iter`).
- MVIR generation must lower this to a runtime call to `.into_iter()` and a `while` loop calling `.next()`, or if those traits aren't available yet, emit a semantic error saying "for loops require IntoIterator trait" rather than silently succeeding.

**Fix 3: Comptime Complex Values**
- If a `Comptime` value is a struct/tuple, it must be serialized into a global constant (e.g., `.data` section via LLVM GlobalVariables) and the MVIR generator should return a pointer to that global.

**Fix 4: Enforce Generic Bounds**
- Extend `SemanticTables` to store bounds: `generic_bounds: HashMap<SymbolId, Vec<SemanticTypeId>>`.
- When typechecking a function call or struct instantiation (`Expr::Call` or `Expr::StructInit`), map generic arguments and verify they implement the required traits using existing `dyn_coercions` / trait resolution infrastructure. Emit `Diagnostic::error` if bounds are not met.

---

## 3. Ordered Sub-Phases & Dependencies

### Sub-Phase 1: "Strict Mode" Hardening (No Silent Failures)
*Dependency: None*
- Strip all `_ => 0` and `_ => false` from `mellis-mvir` and `mellis-backend`.
- Add explicit panics or explicit compiler errors for unhandled AST variants in MVIR generation.
- Ensure the compiler aborts BEFORE MVIR generation if `Diagnostics` contains any errors (e.g. from failed macro expansions).

### Sub-Phase 2: Generic Bounds Enforcement
*Dependency: Sub-Phase 1*
- Parse and record `generic_params` bounds in `TypeChecker`.
- Implement `check_trait_bounds(type_args, bounds)` during instantiation.
- Add `Diagnostic` emissions for bound violations.

### Sub-Phase 3: Control Flow & `for` Loops
*Dependency: Sub-Phase 1*
- Implement MVIR translation for `Stmt::For`. 
- Ensure `Break` and `Continue` correctly jump to the nearest loop's exit/cond block, including handling drops.
- Implement missing LLVM instructions for `BoxNew`, `BoxFree`, `MarkInit`, `PtrCast`, `PtrOffset`, and `SizeOf`.

### Sub-Phase 4: Comptime & Macros
*Dependency: Sub-Phase 3*
- Support generating Global Identifiers for complex Comptime evaluated structs.
- Fix macro expansion pipeline to ensure `MacroCall` never leaks into MVIR.

---

## 4. Affected Crates & Files

- **`mellis-driver`**: `lib.rs` (ensure compilation halts if `diagnostics.has_errors()` before MVIR generation).
- **`mellis-mvir`**: `generator.rs` (remove `_ => 0`, add `Stmt::For`, add explicit `Comptime` handling).
- **`mellis-backend`**: `llvm_codegen.rs` (remove `_ => 0` fallbacks, implement `BoxNew`, `PtrCast`, `SizeOf`).
- **`mellis-semantic`**: `typechecker.rs` (process `generic_params`, bounds checking logic).

---

## 5. Test Matrix

1. **Negative Test (Fallback)**: Write a file with an unsupported instruction (or manual MVIR inject) and ensure the compiler explicitly fails (exit code > 0) rather than emitting `0`.
2. **Negative Test (Bounds)**: Declare `fn test<T: Trait>()` and call `test::<i32>()` where `i32` lacks `Trait`. Ensure compiler emits a semantic error.
3. **Positive Test (For Loop)**: Write a `for` loop over an iterator. Ensure MVIR contains branch and load instructions.
4. **Positive Test (Box)**: Call `BoxNew`. Ensure LLVM bitcode contains a `malloc` call.
5. **Positive Test (Comptime)**: `comptime { struct Foo { x: 1 } }`. Ensure it generates a `.rodata` global variable.

---

## 6. Semantic Invariants to Guarantee

- **Invariant A (Soundness)**: The `mellis-mvir` generator will PANIC or return `Err` if it encounters an AST node it does not explicitly handle. It will never synthesize a default value.
- **Invariant B (Typing)**: The LLVM backend will return a `BackendError` for any MVIR instruction it cannot lower, halting codegen.
- **Invariant C (Contracts)**: Generics can only be instantiated if the typechecker mathematically proves that the provided type arguments satisfy all declared trait bounds.

---

## 7. Risks and Possible Regressions

- **Risk**: Removing the `_ => 0` catch-alls might immediately break existing tests that were silently relying on these fallbacks to compile. 
- **Mitigation**: We must identify and fix the tests that were silently broken.
- **Risk**: Lowering `for` loops requires `IntoIterator` and `Iterator`. If these traits are not defined yet in the `mellis` stdlib, we cannot lower them accurately.
- **Mitigation**: If `Iterator` isn't available, we may temporarily lower `for` into a compiler error ("Stdlib Iterator trait required") until Phase 18, but it MUST NOT be ignored.

---

## 8. Recommended Execution Order for Coding Agent

1. Run a `grep` for `_ =>` in MVIR and LLVM Backend and replace them with `unreachable!()` or `unimplemented!()`. Run the test suite to discover which tests immediately fail.
2. Fix `mellis-driver` to cleanly exit if diagnostics contain errors before calling `generator.generate()`.
3. Implement `Instruction::BoxNew`, `PtrCast`, `SizeOf` in `llvm_codegen.rs` to fix tests broken by step 1.
4. Add generic bound verification to `typechecker.rs`.
5. Address `Stmt::For` and `Expr::Comptime` in `generator.rs`.
